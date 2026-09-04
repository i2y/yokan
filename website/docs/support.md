# Coverage against CPython 3.14.2

Written by `tools/stdlib_coverage.py` from the manifest in `yokan_gate.py` and the translator's own tables. Do not edit by hand.

## The builtins — 16 of the 45 probed

Nothing declares these, so each one was written into a handler the way an app would write it and run past the translator. The note is the refusal's own words. Some that are refused as a value are taken in a `for`, and the note says so.

| name | Yokan | note |
|---|---|---|
| `abs` | ✓ | |
| `all` | — | [1] |
| `any` | — | [1] |
| `bin` | — | [1] |
| `bool` | ✓ | |
| `callable` | — | [1] |
| `chr` | — | [1] |
| `dict` | — | not in the dialect yet |
| `divmod` | ✓ | |
| `enumerate` | ✓ | |
| `filter` | — | [1] |
| `float` | ✓ | |
| `format` | — | [1] |
| `getattr` | — | [1] |
| `hasattr` | — | [1] |
| `hash` | — | [1] |
| `hex` | — | [1] |
| `id` | — | [1] |
| `input` | — | [1] |
| `int` | ✓ | |
| `isinstance` | — | [1] |
| `iter` | — | [1] |
| `len` | ✓ | |
| `list` | — | not in the dialect yet |
| `map` | — | [1] |
| `max` | ✓ | |
| `min` | ✓ | |
| `next` | — | [1] |
| `oct` | — | [1] |
| `open` | — | [1] |
| `ord` | — | [1] |
| `pow` | — | [1] |
| `print` | — | writes to stdout, which is where a headless run's screen dump goes — `log("…")` writes the same line to stderr in both runs |
| `range` | ✓ | |
| `repr` | — | [1] |
| `reversed` | ✓ | |
| `round` | ✓ | |
| `set` | — | not in the dialect yet |
| `setattr` | — | not a handler statement the dialect knows yet — handler statements are state writes (`x.set(v)`, `xs[i] = v`, `d[k] = v`), store and model calls, if/while/for/match, try, and locals |
| `sorted` | ✓ | |
| `str` | ✓ | |
| `sum` | ✓ | |
| `tuple` | — | not in the dialect yet |
| `type` | — | [1] |
| `zip` | ✓ | |

[1] not in the dialect here — expressions are state reads, fields, locals, literals, arithmetic, comparisons, helper calls and method calls

## `fs` — Yokan's own, 11 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `app_dir`, `append_text`, `exists`, `list_dir`, `make_dir`, `open_dialog`, `read_text`, `read_text_or`, `remove`, `save_dialog`, `write_text`

## `sqlite` — Yokan's own, 7 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `exec`, `query_int`, `query_int_or`, `query_rows`, `query_rows_or`, `query_text`, `query_text_or`

## `clipboard` — Yokan's own, 2 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `get_text`, `set_text`

## `notify` — Yokan's own, 1 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `send`

## `http` — 0 of Python's 2

| name | Yokan | note |
|---|---|---|
| `HTTPMethod` | — | |
| `HTTPStatus` | — | |
| `get_text` | ✓ | not in Python's module |
| `get_text_or` | ✓ | not in Python's module |
| `get_text_with` | ✓ | not in Python's module |
| `post_text` | ✓ | not in Python's module |
| `post_text_or` | ✓ | not in Python's module |
| `status` | ✓ | not in Python's module |

## `math` — 56 of Python's 62

| name | Yokan | note |
|---|---|---|
| `acos` | ✓ |  |
| `acosh` | ✓ |  |
| `asin` | ✓ |  |
| `asinh` | ✓ |  |
| `atan` | ✓ |  |
| `atan2` | ✓ |  |
| `atanh` | ✓ |  |
| `cbrt` | ✓ |  |
| `ceil` | ✓ |  |
| `comb` | ✓ |  |
| `copysign` | ✓ |  |
| `cos` | ✓ |  |
| `cosh` | ✓ |  |
| `degrees` | ✓ |  |
| `dist` | ✓ |  |
| `e` | ✓ |  |
| `erf` | — | |
| `erfc` | — | |
| `exp` | ✓ |  |
| `exp2` | ✓ |  |
| `expm1` | ✓ |  |
| `fabs` | ✓ |  |
| `factorial` | ✓ |  |
| `floor` | ✓ |  |
| `fma` | ✓ |  |
| `fmod` | ✓ |  |
| `frexp` | ✓ |  |
| `fsum` | ✓ |  |
| `gamma` | — | |
| `gcd` | ✓ |  |
| `hypot` | ✓ |  |
| `inf` | ✓ |  |
| `isclose` | ✓ |  |
| `isfinite` | ✓ |  |
| `isinf` | ✓ |  |
| `isnan` | ✓ |  |
| `isqrt` | ✓ |  |
| `lcm` | ✓ |  |
| `ldexp` | ✓ |  |
| `lgamma` | — | |
| `log` | ✓ |  |
| `log10` | ✓ |  |
| `log1p` | ✓ |  |
| `log2` | ✓ |  |
| `modf` | ✓ |  |
| `nan` | ✓ |  |
| `nextafter` | ✓ |  |
| `perm` | ✓ |  |
| `pi` | ✓ |  |
| `pow` | ✓ |  |
| `prod` | — | |
| `radians` | ✓ |  |
| `remainder` | ✓ |  |
| `sin` | ✓ |  |
| `sinh` | ✓ |  |
| `sqrt` | ✓ |  |
| `sumprod` | — | |
| `tan` | ✓ |  |
| `tanh` | ✓ |  |
| `tau` | ✓ |  |
| `trunc` | ✓ |  |
| `ulp` | ✓ |  |

## `jsondoc` — Yokan's own, 6 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `get_bool`, `get_float`, `get_int`, `get_text`, `has`, `length`

## `json` — 1 of Python's 5

| name | Yokan | note |
|---|---|---|
| `detect_encoding` | — | |
| `dump` | — | |
| `dumps` | ✓ |  |
| `load` | — | |
| `loads` | — | |

## `strings` — Yokan's own, 2 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `to_float`, `to_int`

## `random` — 9 of Python's 33

| name | Yokan | note |
|---|---|---|
| `BPF` | — | |
| `LOG4` | — | |
| `NV_MAGICCONST` | — | |
| `RECIP_BPF` | — | |
| `Random` | — | |
| `SG_MAGICCONST` | — | |
| `SystemRandom` | — | |
| `TWOPI` | — | |
| `betavariate` | — | |
| `binomialvariate` | — | |
| `choice` | ✓ |  |
| `choices` | — | |
| `expovariate` | — | |
| `gammavariate` | — | |
| `gauss` | ✓ |  |
| `getrandbits` | ✓ |  |
| `getstate` | — | |
| `lognormvariate` | — | |
| `main` | — | |
| `normalvariate` | — | |
| `paretovariate` | — | |
| `randbytes` | — | |
| `randint` | ✓ |  |
| `random` | ✓ |  |
| `randrange` | ✓ |  |
| `sample` | ✓ |  |
| `seed` | ✓ |  |
| `setstate` | — | |
| `shuffle` | — | |
| `triangular` | — | |
| `uniform` | ✓ |  |
| `vonmisesvariate` | — | |
| `weibullvariate` | — | |

## `statistics` — 8 of Python's 25

| name | Yokan | note |
|---|---|---|
| `LinearRegression` | — | |
| `NormalDist` | — | |
| `StatisticsError` | — | |
| `correlation` | — | |
| `covariance` | — | |
| `fmean` | ✓ |  |
| `geometric_mean` | — | |
| `harmonic_mean` | — | |
| `kde` | — | |
| `kde_random` | — | |
| `linear_regression` | — | |
| `mean` | ✓ |  |
| `median` | ✓ |  |
| `median_grouped` | — | |
| `median_high` | — | |
| `median_low` | — | |
| `mode` | ✓ |  |
| `multimode` | — | |
| `pi` | — | |
| `pstdev` | ✓ |  |
| `pvariance` | ✓ |  |
| `quantiles` | — | |
| `stdev` | ✓ |  |
| `tau` | — | |
| `variance` | ✓ |  |

## `time` — 7 of Python's 38

| name | Yokan | note |
|---|---|---|
| `CLOCK_MONOTONIC` | — | |
| `CLOCK_MONOTONIC_RAW` | — | |
| `CLOCK_MONOTONIC_RAW_APPROX` | — | |
| `CLOCK_PROCESS_CPUTIME_ID` | — | |
| `CLOCK_REALTIME` | — | |
| `CLOCK_THREAD_CPUTIME_ID` | — | |
| `CLOCK_UPTIME_RAW` | — | |
| `CLOCK_UPTIME_RAW_APPROX` | — | |
| `altzone` | — | |
| `asctime` | — | |
| `clock_getres` | — | |
| `clock_gettime` | — | |
| `clock_gettime_ns` | — | |
| `clock_settime` | — | |
| `clock_settime_ns` | — | |
| `ctime` | — | |
| `daylight` | — | |
| `get_clock_info` | — | |
| `gmtime` | — | |
| `localtime` | — | |
| `mktime` | — | |
| `monotonic` | ✓ |  |
| `monotonic_ns` | ✓ |  |
| `perf_counter` | ✓ |  |
| `perf_counter_ns` | ✓ |  |
| `process_time` | — | |
| `process_time_ns` | — | |
| `sleep` | ✓ |  |
| `strftime` | — | |
| `strptime` | — | |
| `struct_time` | — | |
| `thread_time` | — | |
| `thread_time_ns` | — | |
| `time` | ✓ |  |
| `time_ns` | ✓ |  |
| `timezone` | — | |
| `tzname` | — | |
| `tzset` | — | |

## `clock` — Yokan's own, 3 functions

Python has no module of this name, so there is nothing to measure against — Yokan has all of them.

- `format_local_ms`, `format_ms`, `local_offset_minutes`

## `string` — 9 of Python's 12

| name | Yokan | note |
|---|---|---|
| `Formatter` | — | |
| `Template` | — | |
| `ascii_letters` | ✓ |  |
| `ascii_lowercase` | ✓ |  |
| `ascii_uppercase` | ✓ |  |
| `capwords` | — | |
| `digits` | ✓ |  |
| `hexdigits` | ✓ |  |
| `octdigits` | ✓ |  |
| `printable` | ✓ |  |
| `punctuation` | ✓ |  |
| `whitespace` | ✓ |  |

## `textwrap` — 2 of Python's 6

| name | Yokan | note |
|---|---|---|
| `TextWrapper` | — | |
| `dedent` | ✓ |  |
| `fill` | — | |
| `indent` | ✓ |  |
| `shorten` | — | |
| `wrap` | — | |

## `bisect` — 3 of Python's 6

| name | Yokan | note |
|---|---|---|
| `bisect` | ✓ |  |
| `bisect_left` | ✓ |  |
| `bisect_right` | ✓ |  |
| `insort` | — | |
| `insort_left` | — | |
| `insort_right` | — | |

## `heapq` — 2 of Python's 13

| name | Yokan | note |
|---|---|---|
| `heapify` | — | |
| `heapify_max` | — | |
| `heappop` | — | |
| `heappop_max` | — | |
| `heappush` | — | |
| `heappush_max` | — | |
| `heappushpop` | — | |
| `heappushpop_max` | — | |
| `heapreplace` | — | |
| `heapreplace_max` | — | |
| `merge` | — | |
| `nlargest` | ✓ |  |
| `nsmallest` | ✓ |  |

## `re` — 7 of Python's 33

| name | Yokan | note |
|---|---|---|
| `A` | — | |
| `ASCII` | — | |
| `DEBUG` | — | |
| `DOTALL` | — | |
| `I` | — | |
| `IGNORECASE` | — | |
| `L` | — | |
| `LOCALE` | — | |
| `M` | — | |
| `MULTILINE` | — | |
| `Match` | — | |
| `NOFLAG` | — | |
| `Pattern` | — | |
| `PatternError` | — | |
| `RegexFlag` | — | |
| `S` | — | |
| `Scanner` | — | |
| `U` | — | |
| `UNICODE` | — | |
| `VERBOSE` | — | |
| `X` | — | |
| `compile` | — | |
| `error` | — | |
| `escape` | ✓ |  |
| `findall` | ✓ |  |
| `finditer` | — | |
| `fullmatch` | ✓ |  |
| `match` | ✓ |  |
| `purge` | — | |
| `search` | ✓ |  |
| `split` | ✓ |  |
| `sub` | ✓ |  |
| `subn` | — | |

## `datetime` — 3 of Python's 10

| name | Yokan | note |
|---|---|---|
| `MAXYEAR` | — | |
| `MINYEAR` | — | |
| `UTC` | — | |
| `date` | ✓ |  |
| `datetime` | ✓ |  |
| `datetime_CAPI` | — | |
| `time` | — | |
| `timedelta` | ✓ |  |
| `timezone` | — | |
| `tzinfo` | — | |

## `collections` — 1 of Python's 10

| name | Yokan | note |
|---|---|---|
| `ChainMap` | — | |
| `Counter` | ✓ |  |
| `OrderedDict` | — | |
| `UserDict` | — | |
| `UserList` | — | |
| `UserString` | — | |
| `defaultdict` | — | |
| `deque` | — | |
| `heapq` | — | |
| `namedtuple` | — | |

## `itertools` — 6 of Python's 20

| name | Yokan | note |
|---|---|---|
| `accumulate` | ✓ |  |
| `batched` | — | |
| `chain` | ✓ |  |
| `combinations` | ✓ |  |
| `combinations_with_replacement` | — | |
| `compress` | — | |
| `count` | — | |
| `cycle` | — | |
| `dropwhile` | — | |
| `filterfalse` | — | |
| `groupby` | — | |
| `islice` | — | |
| `pairwise` | ✓ |  |
| `permutations` | ✓ |  |
| `product` | ✓ |  |
| `repeat` | — | |
| `starmap` | — | |
| `takewhile` | — | |
| `tee` | — | |
| `zip_longest` | — | |

