//! Where the pointer is, as far as a headless run can say it.
//!
//! A chart shows the value under the pointer: the bar's or the
//! sample's label and one number per series. In a window the engine
//! reads the mouse and draws that readout itself. A headless run has
//! no pointer, so the script says where it is — `hover[@n]:<i>` puts
//! it on the i-th point of the n-th chart, `hover:` takes it away —
//! and the dump carries the readout the window would draw. That is
//! what makes the readout a checked output rather than a courtesy of
//! the window: the two runs are held to the same text for the same
//! point.
//!
//! The state is the kernel's, not the element's: an element is rebuilt
//! from the view on every change, and the pointer does not move when
//! the data does.

use crate::{List, Str, World, float_text};

#[derive(Default)]
pub struct Hover {
    /// `(chart, point)`: the n-th chart in tree order — BarChart and
    /// LineChart counted together, the way the script counts them —
    /// and the index of the point under the pointer.
    at: Option<(usize, usize)>,
}

fn store(w: &mut World) -> crate::Handle<Hover> {
    w.singleton::<Hover>(Hover::default)
}

/// The pointer is over point `point` of chart `chart`.
pub fn set(w: &mut World, chart: usize, point: usize) {
    let h = store(w);
    w.get_mut(h).at = Some((chart, point));
}

/// The pointer left.
pub fn clear(w: &mut World) {
    let h = store(w);
    w.get_mut(h).at = None;
}

/// The hovered point of chart `n`, when the pointer is over it.
pub fn point_of(w: &World, n: usize) -> Option<usize> {
    let h = w.try_singleton_ref::<Hover>()?;
    match w.get(h).at {
        Some((c, p)) if c == n => Some(p),
        _ => None,
    }
}

/// The readout a chart shows for point `i`: its label, or `#i` when
/// the chart has none, then one value per series (`data` alone when
/// there are no series, the kernel's rule for what a chart plots).
/// The same float text the dump prints for the data, so the window
/// and the dump say one number.
pub fn chart_readout(
    data: &List<f64>,
    labels: &List<Str>,
    series: &List<List<f64>>,
    i: usize,
) -> String {
    let name = match labels.get(i as i64) {
        Some(l) => l.as_str().to_string(),
        None => format!("#{i}"),
    };
    let values: Vec<String> = if series.is_empty() {
        data.get(i as i64).map(float_text).into_iter().collect()
    } else {
        series
            .iter()
            .filter_map(|s| s.get(i as i64))
            .map(float_text)
            .collect()
    };
    format!("{name}: {}", values.join(", "))
}

/// The number of points chart `n` plots — the longest series, or the
/// data — so a script's point index can be checked before it is set.
pub fn points(data: &List<f64>, series: &List<List<f64>>) -> usize {
    if series.is_empty() {
        data.len()
    } else {
        series.iter().map(List::len).max().unwrap_or(0)
    }
}
