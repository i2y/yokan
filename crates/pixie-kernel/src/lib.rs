// S3 spike: the runtime kernel the generated code targets.
//
// Everything user objects need at runtime lives in one single-threaded World:
// a generational slot map. Generated code never holds &/&mut across statements;
// it goes World -> value and back, so every emitted shape is borrow-clean by
// construction. Handlers and listeners are Rc<dyn Fn(&mut World)> capturing
// only Copy handles and values.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

/// A fast, non-cryptographic hasher for the World's own bookkeeping
/// sets (§8.50). Their keys are `(slot, generation)` pairs the
/// runtime makes up itself — never attacker-supplied — so the
/// default hasher's collision resistance buys nothing and costs a
/// measured 2.3 ns on every property write, which is where a
/// property write's time was going.
#[derive(Default, Clone, Copy)]
pub struct HandleHasher(u64);

impl std::hash::Hasher for HandleHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u32(&mut self, v: u32) {
        // The only shape that actually occurs: an ErasedHandle is two
        // u32s. Mix without looping over bytes.
        self.0 = (self.0 ^ v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0 ^ v).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

#[derive(Default, Clone, Copy)]
pub struct HandleHashBuilder;

impl std::hash::BuildHasher for HandleHashBuilder {
    type Hasher = HandleHasher;
    fn build_hasher(&self) -> HandleHasher {
        HandleHasher(0xcbf2_9ce4_8422_2325)
    }
}

type HandleSet = std::collections::HashSet<ErasedHandle, HandleHashBuilder>;

pub mod a11y;
pub mod anim;
pub mod script;
pub mod theme;
pub use anim::Easing;

pub type SignalId = u32;
pub type Listener = Rc<dyn Fn(&mut World)>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ErasedHandle {
    ix: u32,
    generation: u32,
}

pub struct Handle<T> {
    ix: u32,
    generation: u32,
    _t: PhantomData<T>,
}

// Manual impls: Handle<T> is Copy regardless of T.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}

impl<T> Handle<T> {
    /// A handle that names no object, for the window between a
    /// store's construction and main assigning its real value
    /// (§8.64). Generations start at 0 and only grow, so this one
    /// never matches a live slot — a read through it is the ordinary
    /// stale-handle failure, not a read of a stranger.
    pub const PENDING: Handle<T> = Handle {
        ix: u32::MAX,
        generation: u32::MAX,
        _t: PhantomData,
    };
}

// Identity comparison, independent of T: two handles are the same
// object iff they name the same live slot. A class-typed prop needs
// this — the setter only notifies when the value CHANGED, and for an
// object "changed" means "points somewhere else" (§11.23).
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ix == other.ix && self.generation == other.generation
    }
}
impl<T> Eq for Handle<T> {}

impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ix.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}, gen {})", self.ix, self.generation)
    }
}

impl<T> Handle<T> {
    pub fn erase(self) -> ErasedHandle {
        ErasedHandle {
            ix: self.ix,
            generation: self.generation,
        }
    }
}

impl ErasedHandle {
    // Reflection support (the rung-2 interpreter): re-type an erased handle.
    // The cast itself is unchecked; every access still verifies generation and
    // downcast, so a wrong T surfaces at get (panic) / try_get (None).
    pub fn typed<T>(self) -> Handle<T> {
        Handle {
            ix: self.ix,
            generation: self.generation,
            _t: PhantomData,
        }
    }
}

struct Slot {
    generation: u32,
    /// How many World EDGES point at this object (§8.44): a
    /// class-typed property, an element of a list-of-objects
    /// property, or a root the runtime pins. Not counted: handles in
    /// Rust locals, in closures, or in the element tree — every one
    /// of those is backed by an edge that IS counted, which is the
    /// invariant the whole scheme rests on.
    ///
    /// Zero means "no edge points here", which is where an object
    /// STARTS. `insert` does not free anything, so an object with no
    /// edges simply lives until §8.42's escape analysis reclaims it
    /// or the process ends. Refcounting only ever acts on the
    /// transition to zero from above.
    rc: u32,
    obj: Option<Box<dyn Any>>,
}

/// The outgoing edges of one object, so freeing it can release what
/// it held. Registered per class by the emitter — the same
/// declaration walk that answers "is this property a handle".
pub type EdgeFn = fn(&World, ErasedHandle) -> Vec<ErasedHandle>;

/// A class's `deinit` body (§8.60). Runs once, when the last
/// reference goes and BEFORE the object leaves the World, so it can
/// still read its own properties. The object is freed immediately
/// afterwards whatever the body did — a `deinit` cannot resurrect
/// what it is running for, and a handle it stored somewhere is dead
/// (generational, so a later read traps rather than reads a stranger).
pub type DeinitFn = fn(&mut World, ErasedHandle);

#[derive(Default)]
pub struct World {
    slots: Vec<Slot>,
    free: Vec<u32>,
    listeners: Vec<(ErasedHandle, SignalId, Listener)>,
    signal_queue: Vec<(ErasedHandle, SignalId)>,
    /// The queue's membership, for O(1) collapse of repeated
    /// PROPERTY-CHANGE notifications (§8.43). Kept beside the queue
    /// rather than replacing it: delivery order is the order things
    /// changed, which a set would not preserve.
    signal_pending: std::collections::HashSet<(ErasedHandle, SignalId), HandleHashBuilder>,
    /// Outgoing-edge readers, by concrete type. The emitter
    /// registers one per class that HAS object-valued properties;
    /// classes made only of values register nothing, so a leaf costs
    /// no lookup (§8.44).
    edges: HashMap<TypeId, EdgeFn>,
    /// Listeners for a signal on ANY object, not one named target
    /// (§8.66). A view subscribes this way to classes it can only
    /// reach THROUGH another object, where there is no handle to name
    /// at mount time because the object may not exist yet.
    class_listeners: Vec<(SignalId, Listener)>,
    /// Which signal ids have a class-level listener. `notify` drops a
    /// notification whose target nobody listens to (§8.43), and this
    /// is the second half of that question — one hash lookup on a set
    /// that holds a handful of ids.
    class_signals: std::collections::HashSet<SignalId>,
    /// Per-class `deinit`, keyed the way `edges` is.
    deinits: HashMap<TypeId, DeinitFn>,
    /// Targets that have at least one listener. A notification for
    /// anything else is observationally nothing — `flush` would scan
    /// the listener list, match none, and move on — so it is never
    /// queued (§8.43). This is what keeps a loop over short-lived
    /// objects from queueing one entry per object: those objects are
    /// nobody's business but the method's.
    ///
    /// Invariant that makes the drop EXACT rather than approximate:
    /// generated code only ever connects to an object it has just
    /// created, so a notification can never precede its target's
    /// first `connect`.
    connected: HandleSet,
    dirty_views: Vec<ErasedHandle>,
    singletons: HashMap<TypeId, ErasedHandle>,
    /// Wired by `Runtime::new`; lets sync handlers spawn async tasks
    /// (`w.async_ctx()` / `w.spawn(...)`) without changing their
    /// `Fn(&mut World)` shape.
    async_ctx: Option<AsyncCtx>,
    /// Tasks spawned while the World was borrowed; `Runtime::turn`
    /// drains them before polling (only `turn` ever polls).
    pending_tasks: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>,
}

impl World {
    pub fn new() -> Self {
        World::default()
    }

    pub fn insert<T: 'static>(&mut self, v: T) -> Handle<T> {
        let ix = match self.free.pop() {
            Some(ix) => {
                self.slots[ix as usize].obj = Some(Box::new(v));
                self.slots[ix as usize].rc = 0;
                ix
            }
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    rc: 0,
                    obj: Some(Box::new(v)),
                });
                (self.slots.len() - 1) as u32
            }
        };
        let h = Handle::<T> {
            ix,
            generation: self.slots[ix as usize].generation,
            _t: PhantomData,
        };
        // A newly built object already holds whatever its `init` put
        // in it, so those edges count from birth (§8.44). Doing it
        // here rather than at each construction site means the
        // emitter cannot forget: `Class::new` has no World to retain
        // through, and this is the first moment one exists.
        for e in self.edges_of(h.erase()) {
            self.retain(e);
        }
        h
    }

    // Explicit end of life: bumps the generation so every surviving handle
    // observes staleness (the `T?` surface) instead of dangling.
    pub fn remove<T: 'static>(&mut self, h: Handle<T>) -> Option<T> {
        {
            let slot = self.slots.get(h.ix as usize)?;
            if slot.generation != h.generation || slot.obj.is_none() {
                return None;
            }
        }
        // A scope-end reclaim (§8.42) frees an object the same way a
        // dropped last reference does, so it runs `deinit` the same
        // way (§8.60). An object that had TWO ways to be freed and
        // ran its destructor for only one of them would make the
        // escape analysis observable, which is the one thing it must
        // not be.
        if let Some(f) = self.deinit_of(h.erase()) {
            f(self, h.erase());
        }
        let slot = self.slots.get_mut(h.ix as usize)?;
        if slot.generation != h.generation {
            return None;
        }
        let obj = slot.obj.take()?;
        slot.rc = 0;
        slot.generation += 1;
        self.free.push(h.ix);
        let e = h.erase();
        self.connected.remove(&e);
        self.listeners.retain(|(t, _, _)| *t != e);
        obj.downcast::<T>().ok().map(|b| *b)
    }

    // Generated-code path: the compiler only emits get/get_mut against handles
    // it can prove reachable (fields of live objects, mounted views); user-level
    // maybe-dead access goes through try_get.
    pub fn get<T: 'static>(&self, h: Handle<T>) -> &T {
        let slot = &self.slots[h.ix as usize];
        assert_eq!(slot.generation, h.generation, "stale handle");
        slot.obj
            .as_ref()
            .expect("empty slot")
            .downcast_ref::<T>()
            .expect("handle type mismatch")
    }

    pub fn get_mut<T: 'static>(&mut self, h: Handle<T>) -> &mut T {
        let slot = &mut self.slots[h.ix as usize];
        assert_eq!(slot.generation, h.generation, "stale handle");
        slot.obj
            .as_mut()
            .expect("empty slot")
            .downcast_mut::<T>()
            .expect("handle type mismatch")
    }

    // The `T?` surface: stale or removed resolves to None, QPointer-style.
    pub fn try_get<T: 'static>(&self, h: Handle<T>) -> Option<&T> {
        let slot = self.slots.get(h.ix as usize)?;
        if slot.generation != h.generation {
            return None;
        }
        slot.obj.as_ref()?.downcast_ref::<T>()
    }

    // `store X { ... }`: generated main() initializes every store up front,
    // then views reach it through singleton_ref with only &World.
    pub fn singleton<T: 'static>(&mut self, init: impl FnOnce() -> T) -> Handle<T> {
        if let Some(e) = self.singletons.get(&TypeId::of::<T>()) {
            return Handle {
                ix: e.ix,
                generation: e.generation,
                _t: PhantomData,
            };
        }
        let h = self.insert(init());
        self.singletons.insert(TypeId::of::<T>(), h.erase());
        h
    }

    /// The singleton handle if one exists. Stores are always
    /// initialized before mounting, so `singleton_ref` is right for
    /// them; this is for kernel-owned resources (the animation store)
    /// that only appear once something uses them.
    pub fn try_singleton_ref<T: 'static>(&self) -> Option<Handle<T>> {
        let e = self.singletons.get(&TypeId::of::<T>())?;
        Some(Handle {
            ix: e.ix,
            generation: e.generation,
            _t: PhantomData,
        })
    }

    pub fn singleton_ref<T: 'static>(&self) -> Handle<T> {
        let e = self
            .singletons
            .get(&TypeId::of::<T>())
            .expect("store not initialized: generated main() runs store init before mounting");
        Handle {
            ix: e.ix,
            generation: e.generation,
            _t: PhantomData,
        }
    }

    /// Teach the World how to read one class's outgoing edges.
    /// Called once per class at startup by generated code.
    pub fn register_edges<T: 'static>(&mut self, f: EdgeFn) {
        self.edges.insert(TypeId::of::<T>(), f);
    }

    pub fn register_deinit<T: 'static>(&mut self, f: DeinitFn) {
        self.deinits.insert(TypeId::of::<T>(), f);
    }

    fn deinit_of(&self, h: ErasedHandle) -> Option<DeinitFn> {
        let slot = self.slots.get(h.ix as usize)?;
        if slot.generation != h.generation {
            return None;
        }
        let obj = slot.obj.as_ref()?;
        self.deinits.get(&(**obj).type_id()).copied()
    }

    fn edges_of(&self, h: ErasedHandle) -> Vec<ErasedHandle> {
        let Some(slot) = self.slots.get(h.ix as usize) else {
            return Vec::new();
        };
        if slot.generation != h.generation {
            return Vec::new();
        }
        let Some(obj) = slot.obj.as_ref() else {
            return Vec::new();
        };
        match self.edges.get(&(**obj).type_id()) {
            Some(f) => f(self, h),
            None => Vec::new(),
        }
    }

    /// One more edge points here.
    #[inline]
    pub fn retain(&mut self, h: ErasedHandle) {
        if let Some(slot) = self.slots.get_mut(h.ix as usize) {
            if slot.generation == h.generation && slot.obj.is_some() {
                slot.rc = slot.rc.saturating_add(1);
            }
        }
    }

    /// One fewer edge points here; at zero the object is freed and
    /// whatever it held is released in turn (§8.44).
    ///
    /// Releasing something with no counted edge is a NO-OP rather
    /// than an underflow. That is the safe direction: a retain the
    /// emitter forgot leaks an object, where an underflow would free
    /// a live one. Every failure of this scheme should cost memory,
    /// never correctness.
    pub fn release(&mut self, h: ErasedHandle) {
        // Iterative, not recursive: dropping the head of a long list
        // of objects would otherwise recurse once per element.
        let mut work = vec![h];
        while let Some(h) = work.pop() {
            let Some(slot) = self.slots.get_mut(h.ix as usize) else {
                continue;
            };
            if slot.generation != h.generation || slot.obj.is_none() || slot.rc == 0 {
                continue;
            }
            slot.rc -= 1;
            if slot.rc > 0 {
                continue;
            }
            // Read the outgoing edges BEFORE the object goes away.
            work.extend(self.edges_of(h));
            // And run `deinit` before that, while the object can
            // still read itself (§8.60). Its own count is already 0,
            // so a `release` reaching this object again finds nothing
            // to do — the guard above is what makes that safe.
            if let Some(f) = self.deinit_of(h) {
                f(self, h);
            }
            self.free_slot(h);
        }
    }

    /// Free without touching counts — `remove`'s body, shared with
    /// `release`.
    fn free_slot(&mut self, h: ErasedHandle) {
        if let Some(slot) = self.slots.get_mut(h.ix as usize) {
            if slot.generation == h.generation && slot.obj.is_some() {
                slot.obj = None;
                slot.rc = 0;
                slot.generation += 1;
                self.free.push(h.ix);
            }
        }
        // A freed object can no longer be anyone's signal target.
        self.connected.remove(&h);
        self.listeners.retain(|(t, _, _)| *t != h);
    }

    /// Pin an object against ever being released — stores, and the
    /// mounted view. A root's count never reaches zero because
    /// nothing ever balances this.
    pub fn root(&mut self, h: ErasedHandle) {
        self.retain(h);
    }

    /// The counted-edge total, for tests.
    pub fn rc_of(&self, h: ErasedHandle) -> u32 {
        match self.slots.get(h.ix as usize) {
            Some(s) if s.generation == h.generation => s.rc,
            _ => 0,
        }
    }

    /// Objects currently alive, for tests and for the memory demo.
    pub fn live_objects(&self) -> usize {
        self.slots.iter().filter(|s| s.obj.is_some()).count()
    }

    pub fn connect(&mut self, target: ErasedHandle, sig: SignalId, f: Listener) {
        self.connected.insert(target);
        self.listeners.push((target, sig, f));
    }

    /// Listen for `sig` on every object that can emit it (§8.66).
    /// Signal ids are unique per (class, property), so this is
    /// "whenever any `Tag`'s `n` changes" — deliberately wider than a
    /// named target, and the reason it can be wired statically.
    pub fn connect_class(&mut self, sig: SignalId, f: Listener) {
        self.class_signals.insert(sig);
        self.class_listeners.push((sig, f));
    }

    // Setters call notify; delivery is deferred to flush so a setter never
    // reenters user code while its own &mut World is live.
    /// An EVENT happened. Every call is delivered, because `emit`
    /// means "this occurred" and two occurrences are two facts.
    // Hot: called once per property write from a SEPARATE crate,
    // where a non-generic method cannot inline without whole-program
    // optimization (§8.50).
    #[inline]
    pub fn notify(&mut self, target: ErasedHandle, sig: SignalId) {
        if !self.connected.contains(&target) && !self.class_signals.contains(&sig) {
            return;
        }
        self.signal_queue.push((target, sig));
        self.signal_pending.insert((target, sig));
    }

    /// A property CHANGED. Repeats collapse until the next flush
    /// (§8.43): a value that changed twice before anyone looked has
    /// changed, once — that is the whole content of the signal, and
    /// the flush is where reactions run.
    ///
    /// This is not only about memory, though a loop writing one prop
    /// three million times did queue three million entries. `flush`
    /// scans the listener list per queued pair, so without the
    /// collapse a loop of writes is quadratic in the loop count.
    #[inline]
    pub fn notify_changed(&mut self, target: ErasedHandle, sig: SignalId) {
        if !self.connected.contains(&target) && !self.class_signals.contains(&sig) {
            return;
        }
        if self.signal_pending.insert((target, sig)) {
            self.signal_queue.push((target, sig));
        }
    }

    pub fn flush(&mut self) {
        for _round in 0..64 {
            let queue = std::mem::take(&mut self.signal_queue);
            self.signal_pending.clear();
            if queue.is_empty() {
                return;
            }
            for (target, sig) in queue {
                // Clone the matching Rc listeners first so the borrow of
                // self.listeners ends before any listener runs.
                let mut fired: Vec<Listener> = self
                    .listeners
                    .iter()
                    .filter(|(t, s, _)| *t == target && *s == sig)
                    .map(|(_, _, f)| f.clone())
                    .collect();
                if self.class_signals.contains(&sig) {
                    fired.extend(
                        self.class_listeners
                            .iter()
                            .filter(|(s, _)| *s == sig)
                            .map(|(_, f)| f.clone()),
                    );
                }
                for f in fired {
                    f(self);
                }
            }
        }
        panic!("signal cascade exceeded 64 rounds (feedback loop between setters)");
    }

    /// The re-entry context for async bodies. Panics unless the World
    /// was wrapped by `Runtime::new` (generated mains always do).
    pub fn async_ctx(&self) -> AsyncCtx {
        self.async_ctx
            .clone()
            .expect("async runtime not initialized: wrap the World in Runtime::new first")
    }

    /// Queue a task; it is created here but only ever polled by
    /// `Runtime::turn`, never under a live `&mut World`.
    pub fn spawn(&mut self, fut: impl std::future::Future<Output = ()> + 'static) {
        self.pending_tasks.push(Box::pin(fut));
    }

    #[inline]
    pub fn mark_view_dirty(&mut self, view: ErasedHandle) {
        if !self.dirty_views.contains(&view) {
            self.dirty_views.push(view);
        }
    }

    pub fn take_dirty_views(&mut self) -> Vec<ErasedHandle> {
        std::mem::take(&mut self.dirty_views)
    }
}

// ---------------------------------------------------------------------------
// COW value types: Qt's implicit sharing, spelled in Rust.

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Str(Rc<String>);

impl Str {
    pub fn new() -> Self {
        Str(Rc::new(String::new()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push_str(&mut self, s: &str) {
        Rc::make_mut(&mut self.0).push_str(s);
    }
}

/// COW byte string — Qt's QByteArray role (§11.10): the value that
/// crosses a binding for `Vec<u8>` returns and `&[u8]` params, so
/// `std::fs::read` lands as `Bytes` instead of the 8×-heavier
/// `List<Int>`. Clones share the buffer until mutation (`Str`'s Rc
/// design, byte-flavored).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Bytes(Rc<Vec<u8>>);

impl Bytes {
    pub fn new() -> Self {
        Bytes(Rc::new(Vec::new()))
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Bytes(Rc::new(v))
    }
}

impl From<&[u8]> for Bytes {
    fn from(v: &[u8]) -> Self {
        Bytes(Rc::new(v.to_vec()))
    }
}

/// String concatenation — `a + b` in pixie (`msg += "!"` compounds
/// through it). COW: appends in place when the left side is unshared.
impl std::ops::Add for Str {
    type Output = Str;
    fn add(mut self, rhs: Str) -> Str {
        Rc::make_mut(&mut self.0).push_str(rhs.as_str());
        self
    }
}

/// `s += t` on local `Str` variables (the emitter writes the Rust
/// compound operator directly for locals).
impl std::ops::AddAssign for Str {
    fn add_assign(&mut self, rhs: Str) {
        Rc::make_mut(&mut self.0).push_str(rhs.as_str());
    }
}

impl From<&str> for Str {
    fn from(s: &str) -> Self {
        Str(Rc::new(s.to_string()))
    }
}
impl From<String> for Str {
    fn from(s: String) -> Self {
        Str(Rc::new(s))
    }
}
// Paths convert lossily (macOS/Linux paths are UTF-8 in practice);
// the binding adapters lean on this for PathBuf-returning fns.
impl From<std::path::PathBuf> for Str {
    fn from(p: std::path::PathBuf) -> Self {
        Str(Rc::new(p.to_string_lossy().into_owned()))
    }
}
impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct List<T>(Rc<Vec<T>>);

impl<T> Clone for List<T> {
    fn clone(&self) -> Self {
        List(self.0.clone())
    }
}

impl<T> List<T> {
    pub fn new() -> Self {
        List(Rc::new(Vec::new()))
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T: Clone> List<T> {
    pub fn push(&mut self, v: T) {
        Rc::make_mut(&mut self.0).push(v);
    }
    /// `xs[i]` — the trapping index. Clones out (elements are values
    /// or Copy handles) and takes the language's OWN integer type: a
    /// `usize` parameter forced every emitted index through a cast
    /// the emitter did not write (§11.25). Out of range is a program
    /// error, reported with both numbers rather than as a raw slice
    /// panic, because that message is all the author will see.
    pub fn at(&self, i: i64) -> T {
        let n = self.0.len();
        match usize::try_from(i).ok().filter(|k| *k < n) {
            Some(k) => self.0[k].clone(),
            None => panic!("list index {i} out of range (length {n})"),
        }
    }

    /// `xs[i] = v` — the trapping index in WRITE position (§8.67).
    /// Same contract as `at`: the language's own integer type, and
    /// out of range is a program error naming both numbers.
    pub fn set(&mut self, i: i64, v: T) {
        let n = self.0.len();
        match usize::try_from(i).ok().filter(|k| *k < n) {
            Some(k) => Rc::make_mut(&mut self.0)[k] = v,
            None => panic!("list index {i} out of range (length {n})"),
        }
    }

    /// `xs.get(i)` — the safe peek, `T?`. The name promises this: it
    /// used to panic instead, which made `get` and `first` disagree
    /// about what a lookup does when there is nothing there.
    pub fn get(&self, i: i64) -> Option<T> {
        usize::try_from(i).ok().and_then(|k| self.0.get(k).cloned())
    }

    /// The head as a `T?` — the same contract as `get`, no index.
    pub fn first(&self) -> Option<T> {
        self.0.first().cloned()
    }
}

/// COW ordered map — `Map<K, V>` in pixie (QMap's role, §12 design:
/// the HTTP headers carrier). BTreeMap so key order — and therefore
/// every dump and TAP assertion — is deterministic. Clones share
/// until mutation, `List`'s contract.
#[derive(Debug, PartialEq)]
pub struct Map<K: Ord, V>(Rc<std::collections::BTreeMap<K, V>>);

impl<K: Ord, V> Clone for Map<K, V> {
    fn clone(&self) -> Self {
        Map(self.0.clone())
    }
}

impl<K: Ord, V> Default for Map<K, V> {
    fn default() -> Self {
        Map::new()
    }
}

impl<K: Ord, V> Map<K, V> {
    pub fn new() -> Self {
        Map(Rc::new(std::collections::BTreeMap::new()))
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K: Ord + Clone, V: Clone> Map<K, V> {
    pub fn insert(&mut self, k: K, v: V) {
        Rc::make_mut(&mut self.0).insert(k, v);
    }
    /// Lookup as a `T?` — absent keys are `nil`, never a panic.
    /// `m[k]` — the subscript target. A map lookup answers `V?`
    /// because absence is ordinary here, not a program error: the
    /// list twin (`List::at`) traps instead. Same name on both so the
    /// emitter can lower `x[i]` without knowing which it has —
    /// rustc resolves it by receiver type.
    pub fn at(&self, k: K) -> Option<V> {
        self.get(k)
    }

    pub fn get(&self, k: K) -> Option<V> {
        self.0.get(&k).cloned()
    }
    /// Total lookup: the value or the default. The dialect's
    /// `.get(k, d)` — absence answers the default, never a panic
    /// and never an Option, so it fits any expression position.
    pub fn get_or(&self, k: K, default: V) -> V {
        self.0.get(&k).cloned().unwrap_or(default)
    }
    pub fn contains(&self, k: K) -> bool {
        self.0.contains_key(&k)
    }
    pub fn remove(&mut self, k: K) {
        Rc::make_mut(&mut self.0).remove(&k);
    }
    /// Keys in order, cloned out as a `List` (iteration rides the
    /// existing `for` machinery).
    /// The pairs in key order (§8.68). `BTreeMap` iterates sorted,
    /// which is what lets a map cross to the interpreted tier as a
    /// pair LIST and come back reading the same.
    pub fn pairs(&self) -> Vec<(K, V)> {
        self.0.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub fn keys(&self) -> List<K> {
        self.0.keys().cloned().collect()
    }
    pub fn values(&self) -> List<V> {
        self.0.values().cloned().collect()
    }
}

impl<T: Clone> Default for List<T> {
    fn default() -> Self {
        List::new()
    }
}

/// OS-notification requests. Handler-side code queues them (any
/// tier, any thread-safety context); the ENGINE drains the queue,
/// because only the engine holds a platform handle. Headless runs
/// never drain, so the queue is capped and sending is best-effort
/// by design — a notification is advice, not state.
pub mod notify {
    use std::sync::Mutex;
    static PENDING: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    const CAP: usize = 16;
    pub fn send(title: &str, body: &str) {
        let mut q = PENDING.lock().unwrap();
        if q.len() < CAP {
            q.push((title.to_string(), body.to_string()));
        }
    }
    pub fn drain() -> Vec<(String, String)> {
        std::mem::take(&mut *PENDING.lock().unwrap())
    }
}

// Binding adapters collect converted std-map returns into Maps
// (sorted on the way in — BTreeMap — so the crossing stays
// deterministic no matter what order the HashMap held).
impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for Map<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Map(std::rc::Rc::new(iter.into_iter().collect()))
    }
}

// Binding adapters collect converted Vec returns into Lists.
impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        List(Rc::new(iter.into_iter().collect()))
    }
}

impl<T: fmt::Debug> fmt::Debug for List<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: PartialEq> PartialEq for List<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

// ---------------------------------------------------------------------------
// Engine stub: the element tree a build() pass produces. The real engine
// (GPUI lower half or parts stack) receives this; the spike only needs its
// shape and the handler plumbing.

/// Argful listener for text widgets: the payload is the new text.
pub type TextListener = Rc<dyn Fn(&mut World, Str)>;

/// Argful listener for toggle widgets (Checkbox / Switch): the payload
/// is the NEW checked state — `!checked` at the moment of the click.
pub type BoolListener = Rc<dyn Fn(&mut World, bool)>;

/// Argful listener for value widgets (Slider): the payload is the
/// new value.
pub type FloatListener = Rc<dyn Fn(&mut World, f64)>;

/// Argful listener for the choosers (Select / RadioGroup / TabBar):
/// the payload is the chosen 0-based index.
pub type IntListener = Rc<dyn Fn(&mut World, i64)>;

#[derive(Clone)]
pub enum Element {
    /// The theme scope (§8.37). Produced by the lowerers when an
    /// element carries the universal rider `theme:`, never written
    /// directly in a view; `children` holds exactly the one wrapped
    /// element. Every color token inside it resolves against the
    /// named palette instead of the root one.
    Themed { theme: Str, children: Vec<Element> },
    /// The accessibility wrapper (§8.36). Produced by the lowerers
    /// when an element carries the universal riders `role:` /
    /// `label:`, never written directly in a view; `children` holds
    /// exactly the one wrapped element. An empty `role` means "keep
    /// what the element derives"; an empty `label` means the same for
    /// the name.
    Semantics {
        role: Str,
        label: Str,
        children: Vec<Element>,
    },
    /// The animation wrapper (§8.35). Produced by the lowerers when
    /// an element carries any of the universal riders `animate:` /
    /// `easing:` / `enter:` / `exit:`, never written directly in a
    /// view; `children` holds exactly the one wrapped element (a Vec
    /// so the tree walkers keep treating it like any other
    /// container), and `opacity` is filled by the settle pass rather
    /// than by `build`.
    Anim {
        /// Tween duration in ms. `0.0` = property changes snap; the
        /// enter/exit fades then do nothing either.
        duration: f64,
        easing: Easing,
        /// Fade in the first time this path appears.
        enter: bool,
        /// Retain this subtree and fade it out when it leaves its
        /// parent's child list.
        exit: bool,
        /// `1.0` unless a fade is running. Not authored — the settle
        /// pass owns it, which is why it shows up in dumps.
        opacity: f64,
        children: Vec<Element>,
    },
    /// `font_size` `0.0` = unset (engine default); `color` empty =
    /// unset (theme text color). Colors are hex strings — `#rgb`,
    /// `#rgba`, `#rrggbb`, `#rrggbbaa` — parsed by the engine each
    /// frame; an invalid string falls back to the theme default
    /// rather than erroring (cute's QColor contract).
    Text {
        text: Str,
        font_size: f64,
        color: Str,
        /// "" = leading; "right" / "center" set the horizontal
        /// alignment (the div stretches to the parent's cross size).
        align: Str,
        /// Flex growth along the parent's main axis (0 = content
        /// height) — a grown readout absorbs spare column space.
        grow: f64,
    },
    /// `background` empty = unset (theme accent surface). A custom
    /// background derives its own hover/press tints in the engine;
    /// `hover_background` / `active_background` override those
    /// (cantrip's `:hover` / `:active`, spelled `hover.background:` /
    /// `active.background:` — style-spliceable like any prop).
    Button {
        label: Str,
        background: Str,
        hover_background: Str,
        active_background: Str,
        /// Fixed size in px; 0 = auto (hug the label — the Image
        /// rule). A sized button centers its label.
        width: f64,
        height: f64,
        /// Label text size / color; 0 / "" = theme defaults.
        font_size: f64,
        color: Str,
        /// Flex growth inside the parent row/column (0 = fixed).
        /// `basis` is the flex-basis in px (with grow, `width` is
        /// ignored) — lets grown spans align with gapped columns.
        grow: f64,
        basis: f64,
        /// Box decoration (yokan's crate boundary), shared with the containers: corner
        /// radius in px (0 = the engine's own), border thickness in px
        /// (0 = none), and the border's color ("" = the theme's).
        border_radius: f64,
        border_width: f64,
        border_color: Str,
        on_click: Listener,
    },
    TextField {
        /// The bound value. The engine pushes it into the live editor
        /// only when it changes between rebuilds, so both controlled
        /// (echoed through a prop) and fire-and-forget bindings hold.
        value: Str,
        placeholder: Str,
        on_change: Option<TextListener>,
        on_submit: Option<TextListener>,
    },
    /// `spacing` is the flex gap in px; `-1.0` = unset (the engine's
    /// default gap — 8 px), and `0.0` is honest zero, so a style can
    /// remove the gap. `padding` `0.0` = none (visually identical to
    /// unset). `background` empty = unset (transparent).
    Column {
        spacing: f64,
        padding: f64,
        background: Str,
        /// Flex growth inside the parent (0 = content-sized). On the
        /// ROOT element this is how a view fills the window.
        grow: f64,
        /// Box decoration (yokan's crate boundary). The radius clips the BACKGROUND, so
        /// it lives on the element that paints it rather than in a
        /// wrapper: a wrapper would round a border drawn outside the
        /// fill.
        border_radius: f64,
        border_width: f64,
        border_color: Str,
        children: Vec<Element>,
    },
    Row {
        spacing: f64,
        padding: f64,
        background: Str,
        /// Flex growth inside the parent (0 = content-sized). On the
        /// ROOT element this is how a view fills the window.
        grow: f64,
        border_radius: f64,
        border_width: f64,
        border_color: Str,
        children: Vec<Element>,
    },
    /// A uniform grid: children fill `columns` equally-wide tracks
    /// left to right and wrap onto a new row when one fills up (CSS
    /// grid auto-flow). `spacing` is the gap on BOTH axes (`-1.0` =
    /// unset, the engine's default), and `padding` / `background` /
    /// `grow` mean exactly what they mean on `Column`.
    ///
    /// Track sizing is uniform BY CONSTRUCTION: gpui's grid template
    /// is `repeat(columns, minmax(0, 1fr))`, so per-column widths
    /// (`100px 1fr auto`) are not expressible through the engine
    /// today — a row of unequal columns stays a `Row` with `grow:`
    /// (see DESIGN §11).
    Grid {
        columns: i64,
        /// Explicit equal row tracks. `0` = unset: rows are implicit
        /// and hug their content (taffy's `grid_auto_rows: auto`), so
        /// a grown Grid would leave slack at the bottom — set `rows:`
        /// when the grid should divide its own height, the way
        /// `columns:` divides its width.
        rows: i64,
        spacing: f64,
        padding: f64,
        background: Str,
        grow: f64,
        border_radius: f64,
        border_width: f64,
        border_color: Str,
        children: Vec<Element>,
    },
    /// One grid item that covers more than a single track. Produced by
    /// the lowerers when an element carries `colSpan:` / `rowSpan:`,
    /// never written directly in a view; `children` holds exactly the
    /// one wrapped element (a Vec so the tree walkers keep treating it
    /// like any other container).
    GridCell {
        col_span: i64,
        row_span: i64,
        children: Vec<Element>,
    },
    /// A z-layering container: children overlap, painted in list order
    /// (later children above earlier ones). Child 0 renders in flow
    /// and sizes the Stack; children 1.. overlay its box edge-to-edge
    /// — see the engine arm for how that maps to taffy absolute
    /// positioning. cute_ui's `StackElement` instead positions every
    /// child absolute (all edges 0) and relies on an externally-set
    /// size; pixie's in-flow child 0 sizes the Stack itself so a
    /// `.pix` view never needs an explicit width/height prop (Stack
    /// has none, matching cute_ui's empty `class Stack < Element {}`
    /// — see DESIGN §11).
    Stack(Vec<Element>),
    /// A vertical list of rows. `item_height` of `0.0` means "unset"
    /// (rows size to their content); any positive value pins every row
    /// to that height, cute_ui's fixed-row contract. `virtualized`
    /// asks the engine to render only the visible window, clipped to
    /// `height` pixels (`0.0` = the engine's 320 px default).
    ///
    /// When the list body is exactly one `for` repeater, the compiler
    /// fills `lazy` instead of `children` (`children` stays empty):
    /// rows are then (re)built on demand for just the range the engine
    /// asks for. With `virtualized` set and `lazy` present the engine
    /// uses a true uniform-list viewport; a virtualized list with
    /// static children falls back to the clipped-viewport rendering.
    ListView {
        virtualized: bool,
        item_height: f64,
        height: f64,
        /// `grow: N` — the viewport takes a flex share of its parent
        /// instead of the fixed `height` (which is ignored when grow
        /// is set). `0.0` = unset, keep the sized behavior.
        grow: f64,
        children: Vec<Element>,
        lazy: Option<LazyRows>,
    },
    /// A vertically wheel-scrollable viewport: children stack like a
    /// Column, the box clips them to `height` pixels (`0.0` = the
    /// engine's 320 px default) and the engine paints a draggable
    /// scrollbar thumb over the right edge. cute_ui's ~120 ms scroll
    /// inertia is still deferred (see DESIGN §11).
    ScrollView { height: f64, children: Vec<Element> },
    /// The horizontal twin of `ScrollView`: children lay out like a
    /// Row and the box clips them to its width — so there is no
    /// `height:` to set, only the bottom-edge thumb.
    HScrollView(Vec<Element>),
    /// A raster/vector image loaded from `source` (used as-is, resolved
    /// against the process cwd at runtime — see DESIGN §11 for the
    /// deferred asset-resolution story). `width`/`height` of `0.0` mean
    /// "unset": the engine leaves that axis to the image's intrinsic
    /// size instead of constraining it.
    Image { source: Str, width: f64, height: f64 },
    /// A vector icon rendered from an SVG file at `source` (same
    /// cwd-relative resolution as `Image` — DESIGN §11.15). Unlike
    /// `Image`, an SVG has no intrinsic raster size, so unset (`0.0`)
    /// `width`/`height` fall back to a 24x24 default at the engine
    /// layer rather than leaving the axis unconstrained. The engine
    /// paints it as a single-color mask tinted with the current text
    /// color (gpui's `svg()` primitive), so multi-color source SVGs
    /// lose their authored colors — icon-only, not general vector art.
    Svg { source: Str, width: f64, height: f64 },
    /// A tabular container. Children are expected to be `Row`s: the
    /// first is the header, the rest are data rows (zebra striping).
    /// Pure layout data here — the header/stripe styling is engine-side.
    DataTable(Vec<Element>),
    /// A full-window dim overlay with a centered surface holding the
    /// children. cute_ui's Modal has no props — it is wrapped in an
    /// `if` in the view body; pixie has no lowerable `if` in views yet,
    /// so visibility rides a required bound `open:` Bool instead
    /// (DESIGN §11). Clicks on the dim area are swallowed, never
    /// auto-closing — cute_ui's `ModalElement::dispatchClick` rule.
    Modal { open: bool, children: Vec<Element> },
    /// A bar chart over `data`, normalized by the largest value, with
    /// `labels` printed under the bars (a short `labels` just labels
    /// the leading bars). `width`/`height` of `0.0` mean "unset": the
    /// engine then spans the available width and gives the plot its
    /// default height. cute_ui's eased-on-data-swap tween is
    /// deferred — v0 paints the data directly (see DESIGN §11).
    BarChart {
        data: List<f64>,
        labels: List<Str>,
        width: f64,
        height: f64,
    },
    /// The polyline twin of `BarChart`: same normalization, same
    /// `labels` rule, same `width`/`height` sizing, points joined by a
    /// stroked path.
    LineChart {
        data: List<f64>,
        labels: List<Str>,
        width: f64,
        height: f64,
    },
    /// A horizontal track with a filled portion proportional to
    /// `value`. The engine clamps to [0,1] at paint time; cute_ui's
    /// eased fill-toward-value animation is deferred (v0 renders the
    /// value directly — see DESIGN §11).
    ProgressBar { value: f64 },
    /// An indeterminate busy indicator: a 120° accent arc rotating at
    /// 1 rev/sec over a full background ring, cute_ui's `SpinnerElement`.
    /// `size` of `0.0` means "unset" — the engine falls back to its
    /// 24 px default (the `Svg` rule; an unset box would paint nothing).
    /// The phase is a wall-clock function, so no state a script could
    /// observe rides here.
    Spinner { size: f64 },
    /// A labeled on/off box. `checked` is BOUND data the app owns:
    /// clicking runs `on_toggle` with the NEW value (`!checked`), and
    /// the mark moves only when the app writes that state back — the
    /// controlled-widget rule TextField's `text:` follows.
    Checkbox {
        label: Str,
        checked: bool,
        on_toggle: Option<BoolListener>,
    },
    /// The pill-and-thumb twin of `Checkbox`: identical contract
    /// (label, bound `checked`, `on_toggle` with the new value),
    /// different paint.
    Switch {
        label: Str,
        checked: bool,
        on_toggle: Option<BoolListener>,
    },
    /// A horizontal value control: a track with a draggable thumb over
    /// `[min, max]`. `value` must be BOUND to a Float property (the
    /// lowerers enforce a property read — a literal could never
    /// reflect state), `step` of `0.0` means continuous, and any other
    /// step snaps to multiples counted from `min` (`slider_snap` is
    /// the one shared rounding rule — engine clicks, drags and the
    /// `slide:` script verb all land on identical values). `on_change`
    /// carries the new value; the engine fires it only when the
    /// snapped value actually changed (the TextField `sync` rule).
    Slider {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        on_change: Option<FloatListener>,
    },
    /// A closed dropdown chooser: the control shows the option at
    /// `selected` and clicking an option reports its 0-based index
    /// through `on_select`. The three choosers (Select / RadioGroup /
    /// TabBar) share one contract — a `List<String>` of choices, an
    /// Int for the current one, an index-carrying handler. The
    /// options are DATA, not child elements, so the tree walkers
    /// treat all three as leaves. Whether the option list is popped
    /// open is engine-side transient state keyed by element path
    /// (the TextField rule) — headless scripts choose directly
    /// through `find_chooser`, so no open flag rides here for a dump
    /// to see.
    Select {
        options: List<Str>,
        selected: i64,
        on_select: Option<IntListener>,
    },
    /// The always-open twin: every option visible as a labelled
    /// radio row, the `selected` one marked.
    RadioGroup {
        options: List<Str>,
        selected: i64,
        on_select: Option<IntListener>,
    },
    /// The horizontal chooser: one clickable tab per label, the
    /// `active` one highlighted.
    TabBar {
        labels: List<Str>,
        active: i64,
        on_select: Option<IntListener>,
    },
}

/// Clamp `v` into `[min, max]`, then snap it to the nearest `step`
/// multiple counted from `min` (`step` `0.0` = continuous), then
/// bound it again so a range whose `max` is not itself a multiple
/// cannot round past the edge. Manual `max`/`min` chaining rather
/// than `f64::clamp`: a degenerate `max < min` range degrades
/// deterministically (to `max`) instead of panicking the frame.
pub fn slider_snap(min: f64, max: f64, step: f64, v: f64) -> f64 {
    let v = v.max(min).min(max);
    if step > 0.0 {
        (min + ((v - min) / step).round() * step).max(min).min(max)
    } else {
        v
    }
}

/// The lazy half of a virtualized ListView: `build` produces just the
/// requested row range against the live World, and `len` was computed
/// eagerly at `build()` time. The closure captures only Copy handles
/// and values (generated) or Rc'd AST + tables (interpreted) — the
/// §3.2 capture rule, so re-invoking it is always borrow-clean.
#[derive(Clone)]
pub struct LazyRows {
    pub len: usize,
    pub build: Rc<dyn Fn(&World, std::ops::Range<usize>) -> Vec<Element>>,
}

/// The `dump()` tail a chart's explicit sizing adds. Empty while both
/// axes are unset (`0.0`), so demos that never touched them compare
/// byte-identically across releases and tiers.
fn chart_size(width: f64, height: f64) -> String {
    let mut out = String::new();
    if width != 0.0 {
        out.push_str(&format!(" w={width}"));
    }
    if height != 0.0 {
        out.push_str(&format!(" h={height}"));
    }
    out
}

/// The box-decoration props as dump fragments (yokan's crate boundary). Shared by
/// every element that paints a box so the four render one way, and
/// per-prop so an all-defaults element dumps exactly as it did before
/// the props existed.
fn box_props(radius: f64, width: f64, color: &Str) -> Vec<String> {
    let mut out = Vec::new();
    if radius != 0.0 {
        out.push(format!("radius={radius}"));
    }
    if width != 0.0 {
        out.push(format!("border={width}"));
    }
    if !color.as_str().is_empty() {
        out.push(format!("borderColor={color}"));
    }
    out
}

impl Element {
    /// All-defaults constructors for the styleable variants — the
    /// spikes and tests build trees through these so growing the
    /// style-prop set never ripples through them again. The emitter
    /// and the interpreter write the full struct literals instead
    /// (they carry the spliced style props).
    pub fn text(s: impl Into<Str>) -> Element {
        Element::Text {
            text: s.into(),
            font_size: 0.0,
            color: Str::from(""),
            align: Str::from(""),
            grow: 0.0,
        }
    }

    pub fn column(children: Vec<Element>) -> Element {
        Element::Column {
            spacing: -1.0,
            padding: 0.0,
            background: Str::from(""),
            grow: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::from(""),
            children,
        }
    }

    pub fn row(children: Vec<Element>) -> Element {
        Element::Row {
            spacing: -1.0,
            padding: 0.0,
            background: Str::from(""),
            grow: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::from(""),
            children,
        }
    }

    pub fn grid(columns: i64, children: Vec<Element>) -> Element {
        Element::Grid {
            columns,
            rows: 0,
            spacing: -1.0,
            padding: 0.0,
            background: Str::from(""),
            grow: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::from(""),
            children,
        }
    }

    pub fn button(label: impl Into<Str>, on_click: Listener) -> Element {
        Element::Button {
            label: label.into(),
            background: Str::from(""),
            hover_background: Str::from(""),
            active_background: Str::from(""),
            width: 0.0,
            height: 0.0,
            font_size: 0.0,
            color: Str::from(""),
            grow: 0.0,
            basis: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::from(""),
            on_click,
        }
    }

    /// Render the tree for headless scripts and the tier gate. Takes
    /// the World so lazy ListView rows can be materialized in full —
    /// verification never sees less than the eager tree would show.
    pub fn dump(&self, w: &World) -> String {
        match self {
            // Style props join the parenthesized group only when set
            // (ListView's rule), so unstyled demos dump byte-identically
            // to the pre-style era.
            Element::Text {
                text,
                font_size,
                color,
                align,
                grow,
            } => {
                let mut s = format!("Text({text}");
                if *font_size != 0.0 {
                    s.push_str(&format!(", fontSize={font_size}"));
                }
                if !color.as_str().is_empty() {
                    s.push_str(&format!(", color={color}"));
                }
                if !align.as_str().is_empty() {
                    s.push_str(&format!(", align={align}"));
                }
                if *grow != 0.0 {
                    s.push_str(&format!(", grow={grow}"));
                }
                s.push(')');
                s
            }
            Element::Button {
                label,
                background,
                hover_background,
                active_background,
                width,
                height,
                font_size,
                color,
                grow,
                basis,
                border_radius,
                border_width,
                border_color,
                ..
            } => {
                let mut props: Vec<String> = Vec::new();
                if !background.as_str().is_empty() {
                    props.push(format!("bg={background}"));
                }
                if !hover_background.as_str().is_empty() {
                    props.push(format!("hoverBg={hover_background}"));
                }
                if !active_background.as_str().is_empty() {
                    props.push(format!("activeBg={active_background}"));
                }
                if *width != 0.0 {
                    props.push(format!("width={width}"));
                }
                if *height != 0.0 {
                    props.push(format!("height={height}"));
                }
                if *font_size != 0.0 {
                    props.push(format!("fontSize={font_size}"));
                }
                if !color.as_str().is_empty() {
                    props.push(format!("color={color}"));
                }
                if *grow != 0.0 {
                    props.push(format!("grow={grow}"));
                }
                if *basis != 0.0 {
                    props.push(format!("basis={basis}"));
                }
                props.extend(box_props(*border_radius, *border_width, border_color));
                if props.is_empty() {
                    format!("Button({label})")
                } else {
                    format!("Button({label}, {})", props.join(", "))
                }
            }
            Element::TextField { value, .. } => format!("TextField({})", value),
            Element::Column {
                spacing,
                padding,
                background,
                grow,
                border_radius,
                border_width,
                border_color,
                children,
            }
            | Element::Row {
                spacing,
                padding,
                background,
                grow,
                border_radius,
                border_width,
                border_color,
                children,
            } => {
                let name = if matches!(self, Element::Column { .. }) {
                    "Column"
                } else {
                    "Row"
                };
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                let mut props: Vec<String> = Vec::new();
                if *spacing >= 0.0 {
                    props.push(format!("spacing={spacing}"));
                }
                if *padding != 0.0 {
                    props.push(format!("padding={padding}"));
                }
                if *grow != 0.0 {
                    props.push(format!("grow={grow}"));
                }
                if !background.as_str().is_empty() {
                    props.push(format!("bg={background}"));
                }
                props.extend(box_props(*border_radius, *border_width, border_color));
                if props.is_empty() {
                    format!("{name}[{}]", inner.join(", "))
                } else {
                    format!("{name}({})[{}]", props.join(", "), inner.join(", "))
                }
            }
            Element::Grid {
                columns,
                rows,
                spacing,
                padding,
                background,
                grow,
                border_radius,
                border_width,
                border_color,
                children,
            } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                // `columns` is required, so it always leads the group;
                // the shared style props follow Column's join rule.
                let mut props: Vec<String> = vec![format!("columns={columns}")];
                if *rows != 0 {
                    props.push(format!("rows={rows}"));
                }
                if *spacing >= 0.0 {
                    props.push(format!("spacing={spacing}"));
                }
                if *padding != 0.0 {
                    props.push(format!("padding={padding}"));
                }
                if *grow != 0.0 {
                    props.push(format!("grow={grow}"));
                }
                if !background.as_str().is_empty() {
                    props.push(format!("bg={background}"));
                }
                props.extend(box_props(*border_radius, *border_width, border_color));
                format!("Grid({})[{}]", props.join(", "), inner.join(", "))
            }
            Element::Themed { theme, children } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                format!("Themed({theme})[{}]", inner.join(", "))
            }
            Element::Semantics {
                role,
                label,
                children,
            } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                let mut props: Vec<String> = Vec::new();
                if !role.as_str().is_empty() {
                    props.push(format!("role={role}"));
                }
                if !label.as_str().is_empty() {
                    props.push(format!("label={label}"));
                }
                format!("Semantics({})[{}]", props.join(", "), inner.join(", "))
            }
            Element::Anim {
                duration,
                easing,
                enter,
                exit,
                opacity,
                children,
            } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                let mut props: Vec<String> = vec![format!("{duration}ms"), easing.name().into()];
                if *enter {
                    props.push("enter".into());
                }
                if *exit {
                    props.push("exit".into());
                }
                // Only a RUNNING fade prints: a settled wrapper has to
                // dump like one that never animated, or every demo's
                // baseline would depend on when the clock was read.
                if *opacity != 1.0 {
                    props.push(format!("opacity={opacity}"));
                }
                format!("Anim({})[{}]", props.join(", "), inner.join(", "))
            }
            Element::GridCell {
                col_span,
                row_span,
                children,
            } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                let mut props: Vec<String> = Vec::new();
                if *col_span != 1 {
                    props.push(format!("colSpan={col_span}"));
                }
                if *row_span != 1 {
                    props.push(format!("rowSpan={row_span}"));
                }
                if props.is_empty() {
                    format!("GridCell[{}]", inner.join(", "))
                } else {
                    format!("GridCell({})[{}]", props.join(", "), inner.join(", "))
                }
            }
            Element::Stack(cs) => {
                let inner: Vec<String> = cs.iter().map(|c| c.dump(w)).collect();
                format!("Stack[{}]", inner.join(", "))
            }
            Element::ListView {
                virtualized,
                item_height,
                height,
                grow,
                children,
                lazy,
            } => {
                let mut inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                if let Some(rows) = lazy {
                    inner.extend((rows.build)(w, 0..rows.len).iter().map(|c| c.dump(w)));
                }
                // A plain list keeps the bare `ListView[..]` rendering:
                // the container props only enter the dump once they are
                // set, so untouched demos compare byte-identically. The
                // same rule applies one prop deeper — `height=` joins
                // the parenthesized group only when it is set, so a
                // list that only sets `virtualized:`/`itemHeight:`
                // dumps exactly as it did before `height:` existed.
                if !*virtualized && *item_height == 0.0 && *height == 0.0 && *grow == 0.0 {
                    format!("ListView[{}]", inner.join(", "))
                } else {
                    let mut props =
                        format!("virtualized={virtualized}, itemHeight={item_height}");
                    if *height != 0.0 {
                        props.push_str(&format!(", height={height}"));
                    }
                    if *grow != 0.0 {
                        props.push_str(&format!(", grow={grow}"));
                    }
                    format!("ListView({props})[{}]", inner.join(", "))
                }
            }
            Element::ScrollView { height, children } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                if *height == 0.0 {
                    format!("ScrollView[{}]", inner.join(", "))
                } else {
                    format!("ScrollView(height={height})[{}]", inner.join(", "))
                }
            }
            Element::HScrollView(cs) => {
                let inner: Vec<String> = cs.iter().map(|c| c.dump(w)).collect();
                format!("HScrollView[{}]", inner.join(", "))
            }
            Element::Image {
                source,
                width,
                height,
            } => format!("Image({source} {width}x{height})"),
            Element::Svg {
                source,
                width,
                height,
            } => format!("Svg({source} {width}x{height})"),
            Element::DataTable(cs) => {
                let inner: Vec<String> = cs.iter().map(|c| c.dump(w)).collect();
                format!("DataTable[{}]", inner.join(", "))
            }
            Element::Modal { open, children } => {
                let inner: Vec<String> = children.iter().map(|c| c.dump(w)).collect();
                format!("Modal({open})[{}]", inner.join(", "))
            }
            // The data drives every painted bar/point, so the dump
            // carries both lists verbatim (`List` is Debug). Sizing
            // joins only once it is set — an untouched chart keeps the
            // bare `BarChart(data labels)` rendering (ListView's rule).
            Element::BarChart {
                data,
                labels,
                width,
                height,
            } => format!("BarChart({data:?} {labels:?}{})", chart_size(*width, *height)),
            Element::LineChart {
                data,
                labels,
                width,
                height,
            } => format!("LineChart({data:?} {labels:?}{})", chart_size(*width, *height)),
            Element::ProgressBar { value } => format!("ProgressBar({value})"),
            Element::Spinner { size } => {
                if *size == 0.0 {
                    "Spinner".to_string()
                } else {
                    format!("Spinner({size})")
                }
            }
            // `checked` is the widget's whole meaning, so it always
            // prints — unlike the join-when-set style props.
            Element::Checkbox { label, checked, .. } => {
                format!("Checkbox({label}, checked={checked})")
            }
            Element::Switch { label, checked, .. } => {
                format!("Switch({label}, checked={checked})")
            }
            // The bound value and its range drive what the user sees,
            // so all three always print; `step` joins the group only
            // when set (the per-prop rule — a continuous slider dumps
            // without it).
            Element::Slider {
                value,
                min,
                max,
                step,
                ..
            } => {
                let mut s = format!("Slider(value={value}, min={min}, max={max}");
                if *step != 0.0 {
                    s.push_str(&format!(", step={step}"));
                }
                s.push(')');
                s
            }
            // The choosers render their whole option list — the
            // choices are as semantic as a Button's label — with the
            // current index always shown (it is the widget's state).
            Element::Select {
                options, selected, ..
            } => {
                let inner: Vec<String> =
                    options.iter().map(|o| o.as_str().to_string()).collect();
                format!("Select(selected={selected})[{}]", inner.join(", "))
            }
            Element::RadioGroup {
                options, selected, ..
            } => {
                let inner: Vec<String> =
                    options.iter().map(|o| o.as_str().to_string()).collect();
                format!("RadioGroup(selected={selected})[{}]", inner.join(", "))
            }
            Element::TabBar { labels, active, .. } => {
                let inner: Vec<String> =
                    labels.iter().map(|l| l.as_str().to_string()).collect();
                format!("TabBar(active={active})[{}]", inner.join(", "))
            }
        }
    }

    /// Look through the compiler-produced wrappers to the element
    /// they decorate. `GridCell`, `Anim`, `Semantics` and `Themed`
    /// are all riders on the SAME element and each holds exactly one
    /// child, so any question of the form "what IS this" has to skip
    /// them — otherwise writing `role:` next to `animate:` would make
    /// one of the two stop working depending on which lowered first.
    pub fn inner(&self) -> &Element {
        match self {
            Element::GridCell { children, .. }
            | Element::Anim { children, .. }
            | Element::Semantics { children, .. }
            | Element::Themed { children, .. } => match children.first() {
                Some(c) => c.inner(),
                None => self,
            },
            _ => self,
        }
    }

    pub fn inner_mut(&mut self) -> &mut Element {
        match self {
            Element::GridCell { children, .. }
            | Element::Anim { children, .. }
            | Element::Semantics { children, .. }
            | Element::Themed { children, .. } => match children.first_mut() {
                Some(c) => c.inner_mut(),
                // A wrapper with no child decorates nothing; it is
                // still an element, so hand it back rather than
                // reborrowing what we already moved out of.
                None => unreachable!("the lowerers never emit a childless wrapper"),
            },
            _ => self,
        }
    }

    pub fn find_button(&self, w: &World, label: &str) -> Option<Listener> {
        self.find_button_nth(w, label, 0)
    }

    /// The n-th button carrying `label`, in tree order (`click@n:`).
    /// Rows of identical buttons — a "delete" per row — are only
    /// reachable by position, so the finders that take an index and
    /// the ones that take a label meet here.
    pub fn find_button_nth(&self, w: &World, label: &str, n: usize) -> Option<Listener> {
        let mut skip = n;
        self.find_button_skip(w, label, &mut skip)
    }

    fn find_button_skip(&self, w: &World, label: &str, skip: &mut usize) -> Option<Listener> {
        match self {
            Element::Button { label: l, on_click, .. } if l.as_str() == label => {
                if *skip == 0 {
                    Some(on_click.clone())
                } else {
                    *skip -= 1;
                    None
                }
            }
            Element::Column { children: cs, .. }
            | Element::Row { children: cs, .. }
            | Element::Grid { children: cs, .. }
            | Element::GridCell { children: cs, .. }
            | Element::Anim { children: cs, .. }
            | Element::Semantics { children: cs, .. }
            | Element::Themed { children: cs, .. }
            | Element::Stack(cs)
            | Element::ScrollView { children: cs, .. }
            | Element::HScrollView(cs)
            | Element::DataTable(cs) => cs.iter().find_map(|c| c.find_button_skip(w, label, skip)),
            // The walk ignores `virtualized`: which rows the engine
            // paints is a rendering decision, not a tree edit.
            Element::ListView { children, lazy, .. } => children
                .iter()
                .find_map(|c| c.find_button_skip(w, label, skip))
                .or_else(|| {
                    lazy.as_ref().and_then(|rows| {
                        (rows.build)(w, 0..rows.len)
                            .iter()
                            .find_map(|c| c.find_button_skip(w, label, skip))
                    })
                }),
            // Headless scripts must be able to reach a closed dialog's
            // buttons, so the walk ignores `open` (a rendered-visibility
            // flag, not a tree edit).
            Element::Modal { children, .. } => children.iter().find_map(|c| c.find_button_skip(w, label, skip)),
            _ => None,
        }
    }

    /// The n-th TextField in document order (headless-script targeting):
    /// its (value, on_change, on_submit) triple.
    #[allow(clippy::type_complexity)]
    pub fn find_text_field(
        &self,
        w: &World,
        n: usize,
    ) -> Option<(Str, Option<TextListener>, Option<TextListener>)> {
        fn walk(
            el: &Element,
            w: &World,
            seen: &mut usize,
            n: usize,
        ) -> Option<(Str, Option<TextListener>, Option<TextListener>)> {
            match el {
                Element::TextField {
                    value,
                    on_change,
                    on_submit,
                    ..
                } => {
                    if *seen == n {
                        return Some((value.clone(), on_change.clone(), on_submit.clone()));
                    }
                    *seen += 1;
                    None
                }
                Element::Column { children: cs, .. }
                | Element::Row { children: cs, .. }
                | Element::Grid { children: cs, .. }
                | Element::GridCell { children: cs, .. }
                | Element::Anim { children: cs, .. }
                | Element::Semantics { children: cs, .. }
                | Element::Themed { children: cs, .. }
                | Element::Stack(cs)
                | Element::ScrollView { children: cs, .. }
                | Element::HScrollView(cs)
                | Element::DataTable(cs) => cs.iter().find_map(|c| walk(c, w, seen, n)),
                // Same as `find_button`: virtualization never hides a
                // row from document order.
                Element::ListView { children, lazy, .. } => {
                    if let Some(hit) = children.iter().find_map(|c| walk(c, w, seen, n)) {
                        return Some(hit);
                    }
                    lazy.as_ref().and_then(|rows| {
                        (rows.build)(w, 0..rows.len)
                            .iter()
                            .find_map(|c| walk(c, w, seen, n))
                    })
                }
                // Same relaxation as `find_button`: a closed Modal's
                // fields still count in document order.
                Element::Modal { children, .. } => {
                    children.iter().find_map(|c| walk(c, w, seen, n))
                }
                _ => None,
            }
        }
        let mut seen = 0;
        walk(self, w, &mut seen, n)
    }

    /// The first Checkbox or Switch labeled `label`, in tree order:
    /// its (checked, on_toggle) pair. Headless `click:` targeting —
    /// the script layer asks `find_button` FIRST, so a Button always
    /// outranks a same-labeled toggle.
    #[allow(clippy::type_complexity)]
    pub fn find_toggle(
        &self,
        w: &World,
        label: &str,
    ) -> Option<(bool, Option<BoolListener>)> {
        self.find_toggle_nth(w, label, 0)
    }

    /// The n-th checkbox or switch carrying `label` — `find_button_nth`'s
    /// twin, so `click@n:` reaches a row of identical toggles too.
    pub fn find_toggle_nth(
        &self,
        w: &World,
        label: &str,
        n: usize,
    ) -> Option<(bool, Option<BoolListener>)> {
        let mut skip = n;
        self.find_toggle_skip(w, label, &mut skip)
    }

    fn find_toggle_skip(
        &self,
        w: &World,
        label: &str,
        skip: &mut usize,
    ) -> Option<(bool, Option<BoolListener>)> {
        match self {
            Element::Checkbox {
                label: l,
                checked,
                on_toggle,
            }
            | Element::Switch {
                label: l,
                checked,
                on_toggle,
            } if l.as_str() == label => {
                if *skip == 0 {
                    Some((*checked, on_toggle.clone()))
                } else {
                    *skip -= 1;
                    None
                }
            }
            Element::Column { children: cs, .. }
            | Element::Row { children: cs, .. }
            | Element::Grid { children: cs, .. }
            | Element::GridCell { children: cs, .. }
            | Element::Anim { children: cs, .. }
            | Element::Semantics { children: cs, .. }
            | Element::Themed { children: cs, .. }
            | Element::Stack(cs)
            | Element::ScrollView { children: cs, .. }
            | Element::HScrollView(cs)
            | Element::DataTable(cs) => cs.iter().find_map(|c| c.find_toggle_skip(w, label, skip)),
            // Same relaxations as `find_button`: virtualization and a
            // closed Modal never hide a toggle from a script.
            Element::ListView { children, lazy, .. } => children
                .iter()
                .find_map(|c| c.find_toggle_skip(w, label, skip))
                .or_else(|| {
                    lazy.as_ref().and_then(|rows| {
                        (rows.build)(w, 0..rows.len)
                            .iter()
                            .find_map(|c| c.find_toggle_skip(w, label, skip))
                    })
                }),
            Element::Modal { children, .. } => {
                children.iter().find_map(|c| c.find_toggle_skip(w, label, skip))
            }
            _ => None,
        }
    }

    /// The n-th Slider in document order (headless-script targeting):
    /// its (min, max, step, on_change) tuple — `find_text_field`'s
    /// shape, one widget over.
    #[allow(clippy::type_complexity)]
    pub fn find_slider(
        &self,
        w: &World,
        n: usize,
    ) -> Option<(f64, f64, f64, Option<FloatListener>)> {
        fn walk(
            el: &Element,
            w: &World,
            seen: &mut usize,
            n: usize,
        ) -> Option<(f64, f64, f64, Option<FloatListener>)> {
            match el {
                Element::Slider {
                    min,
                    max,
                    step,
                    on_change,
                    ..
                } => {
                    if *seen == n {
                        return Some((*min, *max, *step, on_change.clone()));
                    }
                    *seen += 1;
                    None
                }
                Element::Column { children: cs, .. }
                | Element::Row { children: cs, .. }
                | Element::Grid { children: cs, .. }
                | Element::GridCell { children: cs, .. }
                | Element::Anim { children: cs, .. }
                | Element::Semantics { children: cs, .. }
                | Element::Themed { children: cs, .. }
                | Element::Stack(cs)
                | Element::ScrollView { children: cs, .. }
                | Element::HScrollView(cs)
                | Element::DataTable(cs) => cs.iter().find_map(|c| walk(c, w, seen, n)),
                // Same as `find_button`: virtualization never hides a
                // row from document order.
                Element::ListView { children, lazy, .. } => {
                    if let Some(hit) = children.iter().find_map(|c| walk(c, w, seen, n)) {
                        return Some(hit);
                    }
                    lazy.as_ref().and_then(|rows| {
                        (rows.build)(w, 0..rows.len)
                            .iter()
                            .find_map(|c| walk(c, w, seen, n))
                    })
                }
                // Same relaxation as `find_button`: a closed Modal's
                // sliders still count in document order.
                Element::Modal { children, .. } => {
                    children.iter().find_map(|c| walk(c, w, seen, n))
                }
                _ => None,
            }
        }
        let mut seen = 0;
        walk(self, w, &mut seen, n)
    }    /// The n-th chooser in document order (headless-script targeting).
    /// Select, RadioGroup and TabBar count TOGETHER — they share one
    /// contract, so `select@n:` numbers them as one family: the
    /// option/label list and the `onSelect` listener.
    #[allow(clippy::type_complexity)]
    pub fn find_chooser(
        &self,
        w: &World,
        n: usize,
    ) -> Option<(List<Str>, Option<IntListener>)> {
        fn walk(
            el: &Element,
            w: &World,
            seen: &mut usize,
            n: usize,
        ) -> Option<(List<Str>, Option<IntListener>)> {
            match el {
                Element::Select {
                    options, on_select, ..
                }
                | Element::RadioGroup {
                    options, on_select, ..
                }
                | Element::TabBar {
                    labels: options,
                    on_select,
                    ..
                } => {
                    if *seen == n {
                        return Some((options.clone(), on_select.clone()));
                    }
                    *seen += 1;
                    None
                }
                Element::Column { children: cs, .. }
                | Element::Row { children: cs, .. }
                | Element::Grid { children: cs, .. }
                | Element::GridCell { children: cs, .. }
                | Element::Anim { children: cs, .. }
                | Element::Semantics { children: cs, .. }
                | Element::Themed { children: cs, .. }
                | Element::Stack(cs)
                | Element::ScrollView { children: cs, .. }
                | Element::HScrollView(cs)
                | Element::DataTable(cs) => cs.iter().find_map(|c| walk(c, w, seen, n)),
                // Same as `find_text_field`: virtualization never
                // hides a row from document order.
                Element::ListView { children, lazy, .. } => {
                    if let Some(hit) = children.iter().find_map(|c| walk(c, w, seen, n)) {
                        return Some(hit);
                    }
                    lazy.as_ref().and_then(|rows| {
                        (rows.build)(w, 0..rows.len)
                            .iter()
                            .find_map(|c| walk(c, w, seen, n))
                    })
                }
                // Same relaxation as `find_button`: a closed Modal's
                // choosers still count in document order.
                Element::Modal { children, .. } => {
                    children.iter().find_map(|c| walk(c, w, seen, n))
                }
                _ => None,
            }
        }
        let mut seen = 0;
        walk(self, w, &mut seen, n)
    }
}

pub trait Component: Clone
 + 'static {
    fn build(&self, w: &World) -> Element;
    /// Mutable pre-build hook: allocate what `build` will only read.
    /// §8.30 row seats (per-row component state) are ensured here —
    /// `build` itself stays pure.
    fn prepare(&self, _w: &mut World) {}
}

/// The engine/runner entry: run the component's mutable `prepare`
/// phase, then the pure `build`. Every rebuild path goes through
/// here so per-row state exists before the tree reads it.
pub fn build_prepared<C: Component>(w: &mut World, view: Handle<C>) -> Element {
    let v = w.get(view).clone();
    v.prepare(w);
    let el = v.build(w);
    // §8.37: token names become hex FIRST, scoped by any `theme:`
    // rider — so §8.35 below sees two concrete endpoints and a theme
    // flip can tween like any other color change.
    let el = theme::resolve(w, el);
    // §8.35: `build` emits TARGETS; the settle pass folds them against
    // the animation store at the current clock and hands back what a
    // frame should paint. It runs here, on the kernel tree, so both
    // tiers and the headless harness get identical answers.
    anim::settle(w, el)
}

/// Per-row component state (§8.30): one World-side seat per stateful
/// component instance inside `for` repeaters, holding row-state
/// handles keyed by the enclosing repeaters' index PATH. Rows are
/// allocated in `prepare` (every driving list's length is known
/// there) and only READ during build. Seats never shrink —
/// index-keyed semantics: a list that shrinks and regrows sees its
/// old row state again, Flutter's no-key ListView rule.
///
/// Nested repeaters (§8.34) key by path rather than by a flattened
/// index on purpose: a flattened `i * inner_len + j` shifts every
/// mapping when the inner list grows, which would hand row (0, 0)'s
/// state to a different row. The spine below grows independently at
/// each level, so an existing path always keeps its handle.
pub struct RowSeat<C: 'static> {
    /// Rows at this level — the innermost repeater's index.
    rows: Vec<Handle<C>>,
    /// One sub-seat per row of the enclosing repeater, for paths
    /// longer than one. Grow-only by the same rule as `rows`.
    nested: Vec<RowSeat<C>>,
}

impl<C: 'static> RowSeat<C> {
    /// Every row handle this seat holds, at any depth. A seat is a
    /// counted holder like any other (§8.47): its rows are edges, so
    /// they stay alive because the seat does, rather than because
    /// nothing happened to release them.
    pub fn edges(&self) -> Vec<ErasedHandle> {
        let mut out = Vec::new();
        self.collect_edges(&mut out);
        out
    }

    fn collect_edges(&self, out: &mut Vec<ErasedHandle>) {
        out.extend(self.rows.iter().map(|h| h.erase()));
        for n in &self.nested {
            n.collect_edges(out);
        }
    }

    pub fn new() -> Self {
        RowSeat {
            rows: Vec::new(),
            nested: Vec::new(),
        }
    }

    fn get(&self, path: &[usize]) -> Option<Handle<C>> {
        match path {
            [] => None,
            [i] => self.rows.get(*i).copied(),
            [i, rest @ ..] => self.nested.get(*i).and_then(|s| s.get(rest)),
        }
    }

    /// Install `h` at `path`. `ensure_row_grid` walks paths in
    /// lexicographic order and only calls this for a vacant slot, so
    /// the innermost index is always exactly `rows.len()`.
    fn put(&mut self, path: &[usize], h: Handle<C>) {
        match path {
            [] => {}
            [i] => {
                if *i == self.rows.len() {
                    self.rows.push(h);
                }
            }
            [i, rest @ ..] => {
                while self.nested.len() <= *i {
                    self.nested.push(RowSeat::new());
                }
                self.nested[*i].put(rest, h);
            }
        }
    }
}

impl<C: 'static> Default for RowSeat<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// Grow `seat` so every index path in the product of `dims` holds a
/// row, creating the missing ones with `make` (which also wires the
/// row's change signals into the seat's own registered signal, so a
/// row write re-renders the view through the ordinary mount
/// subscription). `dims` runs outermost repeater first; an empty
/// list anywhere means the product is empty and nothing is created.
pub fn ensure_row_grid<C: 'static>(
    w: &mut World,
    seat: Handle<RowSeat<C>>,
    dims: &[usize],
    make: impl Fn(&mut World) -> Handle<C>,
) {
    if dims.is_empty() || dims.iter().any(|n| *n == 0) {
        return;
    }
    let mut path = vec![0usize; dims.len()];
    loop {
        if w.get(seat).get(&path).is_none() {
            let h = make(w);
            w.get_mut(seat).put(&path, h);
        }
        // Odometer over the index product, innermost digit first.
        let mut d = dims.len();
        loop {
            if d == 0 {
                return;
            }
            d -= 1;
            path[d] += 1;
            if path[d] < dims[d] {
                break;
            }
            path[d] = 0;
        }
    }
}

/// The row handle at index `path`, if prepared.
pub fn row_at<C: 'static>(w: &World, seat: Handle<RowSeat<C>>, path: &[usize]) -> Handle<C> {
    w.get(seat).get(path).unwrap_or_else(|| {
        panic!(
            "row state {path:?} not prepared — a repeater built more rows than `prepare` saw"
        )
    })
}

/// Erased row read for the interpreter tier.
pub fn row_at_erased<C: 'static>(
    w: &World,
    seat: Handle<RowSeat<C>>,
    path: &[usize],
) -> Option<ErasedHandle> {
    w.get(seat).get(path).map(|h| h.erase())
}

// Mounting wires "any of these signals fired" -> "this view rebuilds".
// The compiler knows the dependency list statically (cute_ui does the same
// wiring in its codegen today).
pub fn mount<C: Component>(
    w: &mut World,
    c: C,
    deps: &[(ErasedHandle, SignalId)],
) -> Handle<C> {
    let h = w.insert(c);
    let hv = h.erase();
    // A mounted view is a ROOT (§8.44): it is what the frame is built
    // from, and its own fields are counted edges, so rooting it pins
    // the whole view-owned graph without enumerating any of it.
    w.root(hv);
    for (target, sig) in deps {
        w.connect(*target, *sig, Rc::new(move |w| w.mark_view_dirty(hv)));
    }
    w.mark_view_dirty(hv);
    h
}

// ---------------------------------------------------------------------------
// Async tier (S5): tasks run on the main loop and reach the World only through
// AsyncCtx::with between awaits. Runtime::turn is the only poller and never
// runs inside a `with` closure or an event handler, so the RefCell below can
// never observe a nested borrow; generated code never touches it directly.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll, Waker};

pub struct Runtime {
    world: Rc<RefCell<World>>,
    tasks: RefCell<Vec<Pin<Box<dyn Future<Output = ()>>>>>,
}

#[derive(Clone)]
pub struct AsyncCtx {
    world: Rc<RefCell<World>>,
}

impl AsyncCtx {
    pub fn with<R>(&self, f: impl FnOnce(&mut World) -> R) -> R {
        f(&mut self.world.borrow_mut())
    }
}

impl Runtime {
    /// Wrap the World for the async tier and hand it its own re-entry
    /// context (the one deliberate Rc cycle; the World lives for the
    /// process anyway).
    pub fn new(world: World) -> Self {
        let rt = Runtime {
            world: Rc::new(RefCell::new(world)),
            tasks: RefCell::new(Vec::new()),
        };
        let ctx = AsyncCtx {
            world: rt.world.clone(),
        };
        rt.world.borrow_mut().async_ctx = Some(ctx);
        rt
    }

    pub fn ctx(&self) -> AsyncCtx {
        AsyncCtx {
            world: self.world.clone(),
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut World) -> R) -> R {
        f(&mut self.world.borrow_mut())
    }

    pub fn spawn(&self, fut: impl Future<Output = ()> + 'static) {
        self.tasks.borrow_mut().push(Box::pin(fut));
    }

    /// Are any tasks live (queued in the World or held here)?
    pub fn has_tasks(&self) -> bool {
        !self.tasks.borrow().is_empty() || !self.world.borrow().pending_tasks.is_empty()
    }

    // One main-loop turn: adopt World-queued spawns, then poll every
    // live task once. The executor round-robins with a noop waker;
    // completions arrive from workers between turns, and the driving
    // loop (engine pump / headless settle loop) re-turns while
    // `has_tasks`. Returns how many tasks are still pending.
    pub fn turn(&self) -> usize {
        {
            let mut w = self.world.borrow_mut();
            let pending = std::mem::take(&mut w.pending_tasks);
            self.tasks.borrow_mut().extend(pending);
        }
        let mut tasks = self.tasks.take();
        let waker = Waker::noop();
        let mut cx = TaskContext::from_waker(waker);
        tasks.retain_mut(|t| {
            // A panicking task is dropped, printed, and the loop
            // lives on — matching the Python tier's task policy.
            contain("task", || t.as_mut().poll(&mut cx))
                .is_some_and(|p| p.is_pending())
        });
        // Tasks spawned while polling landed in self.tasks (or in the
        // World's queue for the next turn); keep both.
        let mut current = self.tasks.borrow_mut();
        tasks.append(&mut current);
        *current = tasks;
        current.len() + self.world.borrow().pending_tasks.len()
    }
}

/// Run HANDLER code so a panic inside it is printed and contained
/// instead of killing the process — the same containment the Python
/// tier gives handler exceptions, so both tiers observe "this
/// statement failed, earlier effects kept, the app keeps running".
/// Build panics stay loud on purpose: they indicate compiler bugs,
/// not app-reachable states.
pub fn contain<R>(what: &str, f: impl FnOnce() -> R) -> Option<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            eprintln!("pixie: contained {what} error: {msg}");
            None
        }
    }
}

// One-shot completion: the awaitable half is what generated async bodies
// wait on; the handle half is filled by workers (or tests) outside the
// poll — Arc/Mutex so it crosses the worker-thread boundary.
pub struct Completion<T> {
    slot: std::sync::Arc<std::sync::Mutex<Option<T>>>,
}

pub struct CompletionHandle<T> {
    slot: std::sync::Arc<std::sync::Mutex<Option<T>>>,
}

pub fn completion<T>() -> (CompletionHandle<T>, Completion<T>) {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
    (
        CompletionHandle { slot: slot.clone() },
        Completion { slot },
    )
}

impl<T> CompletionHandle<T> {
    pub fn complete(&self, v: T) {
        *self.slot.lock().expect("completion slot poisoned") = Some(v);
    }
}

impl<T> Future for Completion<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext) -> Poll<T> {
        match self.slot.lock().expect("completion slot poisoned").take() {
            Some(v) => Poll::Ready(v),
            None => Poll::Pending,
        }
    }
}

type WorkerSpawner = Box<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;
static WORKER_SPAWNER: std::sync::OnceLock<WorkerSpawner> = std::sync::OnceLock::new();

/// Install the engine's worker pool — the gpui engine hands its
/// background executor in here, so awaited binding calls ride the
/// engine's thread pool instead of thread-per-call. First install
/// wins; headless runs never install and use the fallback.
pub fn set_worker_spawner(s: impl Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static) {
    let _ = WORKER_SPAWNER.set(Box::new(s));
}

/// Run blocking work off the main thread; the closure completes a
/// `CompletionHandle` when done. Uses the installed engine pool, or
/// thread-per-call when none was installed.
pub fn spawn_worker(f: impl FnOnce() + Send + 'static) {
    match WORKER_SPAWNER.get() {
        Some(s) => s(Box::new(f)),
        None => {
            std::thread::spawn(f);
        }
    }
}

/// Battery helper for `.rpi` bindings: blocking sleep, meant to be
/// awaited (`await Clock.sleepMs(300)`) so it runs on a worker.
pub fn sleep_ms(ms: i64) {
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
}

/// Battery helper for `.rpi` bindings: an environment variable's
/// value, or none when unset/non-UTF-8 — the first real `Option`
/// crossing (§11.11's `T?`).
pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Battery helper: join path components with the platform separator.
/// The first binding to take a LIST (§8.73) — arguments crossed as
/// scalars only until the adapter learned the shape the return side
/// had produced all along.
pub fn join_path(parts: Vec<String>) -> String {
    let mut p = std::path::PathBuf::new();
    for part in parts {
        p.push(part);
    }
    p.display().to_string()
}

/// Battery helper: `value` when it is present, `fallback` otherwise.
/// The first binding to TAKE a `T?` — `env_var` has returned one
/// since §11.11, and nothing could hand one back.
pub fn or_else(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

/// How a path exists, as a Rust enum a `.rpi` can correspond to
/// (§8.74). Deliberately a real enum rather than a string: crossing
/// one was the last thing a binding could not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    Missing,
    File,
    Dir,
}

/// Battery helper: what is at `path`.
pub fn path_kind(path: &str) -> PathKind {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        PathKind::Dir
    } else if p.is_file() {
        PathKind::File
    } else {
        PathKind::Missing
    }
}

/// Battery helper: the word for a kind — a binding TAKING an enum.
pub fn kind_name(kind: PathKind) -> String {
    match kind {
        PathKind::Missing => "missing".to_string(),
        PathKind::File => "file".to_string(),
        PathKind::Dir => "dir".to_string(),
    }
}

/// A plain Rust struct a `.rpi` can correspond to field for field
/// (§8.77), the struct twin of `PathKind`. Every field is a type that
/// crosses in BOTH directions — a `u64` would read back fine and then
/// fail on the way out, so rpi-gen would skip the whole struct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileStat {
    /// A `u64`, as the filesystem reports it. Reading widens on its
    /// own; the `.rpi` names the width so the field can be written
    /// back too (§8.78).
    pub len: u64,
    pub readonly: bool,
}

/// Battery helper: size and writability in one crossing — a binding
/// RETURNING a struct.
pub fn file_stat(path: &str) -> FileStat {
    match std::fs::metadata(path) {
        Ok(m) => FileStat {
            len: m.len(),
            readonly: m.permissions().readonly(),
        },
        Err(_) => FileStat {
            len: 0,
            readonly: false,
        },
    }
}

/// Battery helper: a sentence about one — a binding TAKING a struct.
pub fn stat_line(stat: FileStat) -> String {
    let mode = if stat.readonly { "read-only" } else { "writable" };
    format!("{} bytes, {mode}", stat.len)
}

/// One directory entry: a name, a kind, and a stat. A field crosses
/// by the same rule the whole value does, so this one holds both an
/// enum and another struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub kind: PathKind,
    pub stat: FileStat,
}

/// Battery helper: one entry per name, sorted — a binding returning a
/// LIST of structs.
pub fn dir_stats(path: &str) -> Vec<Entry> {
    let mut names: Vec<String> = match std::fs::read_dir(path) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => return Vec::new(),
    };
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let full = format!("{path}/{name}");
            Entry {
                kind: path_kind(&full),
                stat: file_stat(&full),
                name,
            }
        })
        .collect()
}

/// Battery helper: total bytes, optionally of one kind only — a
/// binding TAKING a list of structs beside an optional enum.
pub fn stat_total(entries: Vec<Entry>, only: Option<PathKind>) -> i64 {
    entries
        .iter()
        .filter(|e| only.is_none_or(|k| k == e.kind))
        .map(|e| e.stat.len as i64)
        .sum()
}

/// A file's permission bits, as a newtype over the width the OS
/// uses. A TUPLE struct crosses by position (§8.78): pixie names the
/// field, Rust reaches it as `.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perms(pub u32);

/// Battery helper: the mode bits at `path`, or zero when there are
/// none to read.
pub fn perms_of(path: &str) -> Perms {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(path) {
            return Perms(m.permissions().mode() & 0o777);
        }
    }
    Perms(0)
}

/// Battery helper: those bits as `rwxr-xr-x`.
pub fn perms_text(p: Perms) -> String {
    let mut out = String::with_capacity(9);
    for shift in [6, 3, 0] {
        let bits = (p.0 >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
}

/// A pixie value's Send twin for crossing the worker boundary
/// (`Str` holds an `Rc`; numbers are their own twins). Drives the
/// generic `Map` carriers below.
pub trait Wire: Sized {
    type W: Send + Clone;
    fn to_wire(&self) -> Self::W;
    fn from_wire(w: &Self::W) -> Self;
}

impl Wire for Str {
    type W = String;
    fn to_wire(&self) -> String {
        self.as_str().to_string()
    }
    fn from_wire(w: &String) -> Str {
        Str::from(w.as_str())
    }
}

macro_rules! copy_wire {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            type W = $t;
            fn to_wire(&self) -> $t {
                *self
            }
            fn from_wire(w: &$t) -> $t {
                *w
            }
        }
    )*};
}
copy_wire!(i64, f64, bool);

/// Async-leg carriers for `Map<K, V>` binding args: the worker
/// thread needs `Send`, `Map` holds an `Rc`. The emitter converts to
/// plain pairs main-side, rebuilds on the worker (§12.3) — generic
/// over every wire-able key/value.
pub fn map_to_send<K: Wire + Ord + Clone, V: Wire + Clone>(
    m: &Map<K, V>,
) -> Vec<(K::W, V::W)> {
    m.keys()
        .iter()
        .filter_map(|k| m.get(k.clone()).map(|v| (k.to_wire(), v.to_wire())))
        .collect()
}

pub fn map_from_send<K: Wire + Ord + Clone, V: Wire + Clone>(
    pairs: &[(K::W, V::W)],
) -> Map<K, V> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(K::from_wire(k), V::from_wire(v));
    }
    m
}

/// The HTTP client battery (§12.3): blocking `ureq` calls, made for
/// the worker pool — `await Http.get(...)` ships them off the UI
/// thread; sync calls work too (and block, as they say). One shared
/// agent, 30 s timeout. 4xx/5xx are errors (ureq's model), carrying
/// the status in the message. Response bodies cap at ureq's 10 MB
/// `into_string` limit; `get_bytes` reads unbounded.
pub mod http {
    use super::{Bytes, Map, Str};
    use std::sync::OnceLock;
    use std::time::Duration;

    fn agent() -> &'static ureq::Agent {
        static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
        AGENT.get_or_init(|| {
            ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(30))
                .build()
        })
    }

    fn apply_headers(mut req: ureq::Request, headers: &Map<Str, Str>) -> ureq::Request {
        for k in headers.keys().iter() {
            if let Some(v) = headers.get(k.clone()) {
                req = req.set(k.as_str(), v.as_str());
            }
        }
        req
    }

    pub fn get(url: &str) -> Result<String, ureq::Error> {
        Ok(agent().get(url).call()?.into_string()?)
    }

    pub fn get_bytes(url: &str) -> Result<Vec<u8>, ureq::Error> {
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut agent().get(url).call()?.into_reader(), &mut out)?;
        Ok(out)
    }

    pub fn post(url: &str, body: &str) -> Result<String, ureq::Error> {
        Ok(agent().post(url).send_string(body)?.into_string()?)
    }

    pub fn get_with(url: &str, headers: Map<Str, Str>) -> Result<String, ureq::Error> {
        Ok(apply_headers(agent().get(url), &headers)
            .call()?
            .into_string()?)
    }

    pub fn post_with(
        url: &str,
        body: &str,
        headers: Map<Str, Str>,
    ) -> Result<String, ureq::Error> {
        Ok(apply_headers(agent().post(url), &headers)
            .send_string(body)?
            .into_string()?)
    }

    /// Keep the doc surface honest for rpi-gen: `Bytes` is the
    /// crossing type for `get_bytes` — referenced so the module's
    /// rustdoc JSON names it.
    #[allow(dead_code)]
    fn _crossing(_b: Bytes) {}
}

/// Battery helper for `.rpi` bindings: the entry names inside a
/// directory, sorted — deterministic for scripts and the tier gate.
pub fn list_dir(path: &str) -> std::io::Result<Vec<String>> {
    let mut out: Vec<String> = std::fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    struct Thing {
        v: i64,
    }

    /// §8.42's premise: a removed slot is REUSED, so a method that
    /// creates and reclaims in a loop holds one slot, not N.
    #[test]
    fn a_removed_slot_is_reused() {
        let mut w = World::new();
        let first = w.insert(Thing { v: 0 });
        let ix = first.ix;
        for i in 0..1000 {
            let h = w.insert(Thing { v: i });
            assert_eq!(w.get(h).v, i);
            w.remove(h);
        }
        // The 1000 temporaries all landed in the same slot, which is
        // the one the first insert did NOT take.
        assert_eq!(w.slots.len(), 2, "the World grew past two slots");
        assert_eq!(w.get(first).v, 0, "the survivor is untouched");
        assert_eq!(first.ix, ix);
    }

    /// And the failure mode is loud: a handle to a removed object
    /// observes staleness rather than reading whatever landed there
    /// next. This is what makes §8.42 safe to be wrong about.
    #[test]
    fn a_stale_handle_never_reads_its_successor() {
        let mut w = World::new();
        let dead = w.insert(Thing { v: 1 });
        w.remove(dead);
        let live = w.insert(Thing { v: 2 });
        assert_eq!(live.ix, dead.ix, "the slot was reused");
        assert!(w.try_get(dead).is_none(), "the old handle is stale");
        assert_eq!(w.get(live).v, 2);
    }

    /// §8.44: a counted edge keeps its target alive, and dropping the
    /// last one frees it — along with whatever it held, transitively.
    #[test]
    fn releasing_the_last_edge_frees_the_object_and_its_own() {
        struct Holder {
            kid: ErasedHandle,
        }
        let mut w = World::new();
        w.register_edges::<Holder>(|w, h| vec![w.get(h.typed::<Holder>()).kid]);

        let leaf = w.insert(Thing { v: 7 });
        let owner = w.insert(Holder { kid: leaf.erase() });
        // `insert` counted the edge the new object arrived holding.
        assert_eq!(w.rc_of(leaf.erase()), 1, "the leaf gained an edge at birth");

        w.root(owner.erase());
        assert_eq!(w.live_objects(), 2);

        // Dropping the owner's only edge takes the leaf with it.
        w.release(owner.erase());
        assert_eq!(w.live_objects(), 0, "the cascade reached the leaf");
        assert!(w.try_get(leaf).is_none());
    }

    /// A second edge keeps it: the count is edges, not owners.
    #[test]
    fn a_second_edge_keeps_the_object() {
        struct Holder {
            kid: ErasedHandle,
        }
        let mut w = World::new();
        w.register_edges::<Holder>(|w, h| vec![w.get(h.typed::<Holder>()).kid]);
        let leaf = w.insert(Thing { v: 1 });
        let a = w.insert(Holder { kid: leaf.erase() });
        let b = w.insert(Holder { kid: leaf.erase() });
        // Both holders need a counted edge of their own, or releasing
        // them is a no-op — an object at the top of a graph is either
        // rooted or held by something that is.
        w.root(a.erase());
        w.root(b.erase());
        assert_eq!(w.rc_of(leaf.erase()), 2);
        w.release(a.erase());
        assert!(w.try_get(leaf).is_some(), "b still names it");
        w.release(b.erase());
        assert!(w.try_get(leaf).is_none());
    }

    /// Releasing something with no counted edge does nothing. A
    /// retain the emitter forgot must cost memory, never correctness.
    #[test]
    fn releasing_an_uncounted_object_is_a_no_op() {
        let mut w = World::new();
        let h = w.insert(Thing { v: 1 });
        w.release(h.erase());
        w.release(h.erase());
        assert!(w.try_get(h).is_some(), "an uncounted object is not freed");
    }

    /// §8.66: a class-level listener fires for a signal on ANY
    /// object, which is how a view subscribes to something it can
    /// only reach through another object — there is no handle to name
    /// when the wiring is laid down.
    #[test]
    fn a_class_listener_fires_for_any_target() {
        #[derive(Clone)]
        struct Tag {
            n: i64,
        }
        const TAG_N: SignalId = 9;

        let mut w = World::new();
        let a = w.insert(Tag { n: 0 });
        let b = w.insert(Tag { n: 0 });

        let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let f = fired.clone();
        w.connect_class(TAG_N, Rc::new(move |_| f.set(f.get() + 1)));

        // Neither object is `connect`ed by name, and both are heard.
        w.notify_changed(a.erase(), TAG_N);
        w.notify_changed(b.erase(), TAG_N);
        w.flush();
        assert_eq!(fired.get(), 2);

        // A signal nobody listens for is still dropped before the
        // queue (§8.43) — the class set is the second half of that
        // question, not a replacement for it.
        w.notify_changed(a.erase(), TAG_N + 1);
        w.flush();
        assert_eq!(fired.get(), 2);
    }

    /// §8.60: `deinit` runs once, before the object leaves, and for
    /// both ways an object is freed — the last reference dropping and
    /// an explicit `remove` (which is what a scope-end reclaim uses).
    #[test]
    fn deinit_runs_once_on_both_free_paths() {
        thread_local! {
            static SEEN: std::cell::RefCell<Vec<i64>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        #[derive(Clone)]
        struct Leaf {
            v: i64,
        }
        let mut w = World::new();
        w.register_deinit::<Leaf>(|w, h| {
            // The object is still readable here — that is the point.
            let v = w.get(h.typed::<Leaf>()).v;
            SEEN.with(|s| s.borrow_mut().push(v));
        });

        // Path 1: the last reference goes.
        let a = w.insert(Leaf { v: 1 });
        w.retain(a.erase());
        w.release(a.erase());
        // Path 2: an explicit remove (the scope-end reclaim).
        let b = w.insert(Leaf { v: 2 });
        w.remove(b);

        assert_eq!(SEEN.with(|s| s.borrow().clone()), vec![1, 2]);
        assert_eq!(w.live_objects(), 0);

        // Releasing a handle whose object is already gone runs nothing.
        w.release(a.erase());
        assert_eq!(SEEN.with(|s| s.borrow().len()), 2);
    }

    /// Freeing an object retires its listeners: nothing can be
    /// delivered to a slot that no longer holds it.
    #[test]
    fn freeing_retires_the_listeners() {
        let mut w = World::new();
        let h = w.insert(Thing { v: 0 });
        w.connect(h.erase(), 1, Rc::new(|_| {}));
        w.root(h.erase());
        assert_eq!(w.listeners.len(), 1);
        w.release(h.erase());
        assert!(w.listeners.is_empty(), "a freed object listens to nothing");
        assert!(!w.connected.contains(&h.erase()));
    }

    /// §8.43: a notification nobody listens for is not queued. Before
    /// this, a loop writing a property queued one entry per write —
    /// unbounded memory, and a quadratic flush.
    #[test]
    fn an_unlistened_notification_is_dropped() {
        let mut w = World::new();
        let lonely = w.insert(Thing { v: 0 });
        for _ in 0..10_000 {
            w.notify_changed(lonely.erase(), 1);
            w.notify(lonely.erase(), 2);
        }
        assert!(w.signal_queue.is_empty(), "nothing was listening");
    }

    /// A property that changed twice before anyone looked changed
    /// once. An `emit` is an event, and every one is delivered.
    #[test]
    fn repeated_changes_collapse_but_events_do_not() {
        let mut w = World::new();
        let h = w.insert(Thing { v: 0 });
        w.connect(h.erase(), 1, Rc::new(|_| {}));
        w.connect(h.erase(), 2, Rc::new(|_| {}));
        for _ in 0..100 {
            w.notify_changed(h.erase(), 1);
        }
        assert_eq!(w.signal_queue.len(), 1, "changes collapse");
        for _ in 0..100 {
            w.notify(h.erase(), 2);
        }
        assert_eq!(w.signal_queue.len(), 101, "events do not");
    }

    /// The collapse lasts until the flush, not forever.
    #[test]
    fn a_flush_reopens_the_collapse() {
        let mut w = World::new();
        let h = w.insert(Thing { v: 0 });
        let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let f = fired.clone();
        w.connect(h.erase(), 1, Rc::new(move |_| f.set(f.get() + 1)));
        w.notify_changed(h.erase(), 1);
        w.notify_changed(h.erase(), 1);
        w.flush();
        assert_eq!(fired.get(), 1);
        w.notify_changed(h.erase(), 1);
        w.flush();
        assert_eq!(fired.get(), 2, "a later change is a new notification");
    }
}
