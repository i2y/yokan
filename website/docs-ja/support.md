# CPython 3.14.2 に対する対応状況

`tools/stdlib_coverage.py` が `yokan_gate.py` のマニフェストと翻訳器のテーブルから生成しています。手で編集しないでください。

## 組み込み関数 — 調べた 45 個のうち 16 個

組み込み関数はどこにも宣言されていないので、アプリが書くとおりにハンドラへ書いて翻訳器に通しました。備考は拒否の文言そのまま（英語）です。値としては断られても `for` でなら通るものがあり、それも備考に出ます。

| 名前 | Yokan | 備考 |
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

## `fs` — Yokan 独自、11 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `app_dir`, `append_text`, `exists`, `list_dir`, `make_dir`, `open_dialog`, `read_text`, `read_text_or`, `remove`, `save_dialog`, `write_text`

## `sqlite` — Yokan 独自、7 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `exec`, `query_int`, `query_int_or`, `query_rows`, `query_rows_or`, `query_text`, `query_text_or`

## `clipboard` — Yokan 独自、2 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `get_text`, `set_text`

## `notify` — Yokan 独自、1 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `send`

## `http` — Python の 2 個のうち 0 個

| 名前 | Yokan | 備考 |
|---|---|---|
| `HTTPMethod` | — | |
| `HTTPStatus` | — | |
| `get_text` | ✓ | Python のモジュールにはない |
| `get_text_or` | ✓ | Python のモジュールにはない |
| `get_text_with` | ✓ | Python のモジュールにはない |
| `post_text` | ✓ | Python のモジュールにはない |
| `post_text_or` | ✓ | Python のモジュールにはない |
| `status` | ✓ | Python のモジュールにはない |

## `math` — Python の 62 個のうち 56 個

| 名前 | Yokan | 備考 |
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

## `jsondoc` — Yokan 独自、6 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `get_bool`, `get_float`, `get_int`, `get_text`, `has`, `length`

## `json` — Python の 5 個のうち 1 個

| 名前 | Yokan | 備考 |
|---|---|---|
| `detect_encoding` | — | |
| `dump` | — | |
| `dumps` | ✓ |  |
| `load` | — | |
| `loads` | — | |

## `strings` — Yokan 独自、2 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `to_float`, `to_int`

## `random` — Python の 33 個のうち 9 個

| 名前 | Yokan | 備考 |
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

## `statistics` — Python の 25 個のうち 8 個

| 名前 | Yokan | 備考 |
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

## `time` — Python の 38 個のうち 7 個

| 名前 | Yokan | 備考 |
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

## `clock` — Yokan 独自、3 個

Python に同じ名前のモジュールはないので、比べる相手がありません。以下はすべて Yokan にあります。

- `format_local_ms`, `format_ms`, `local_offset_minutes`

## `string` — Python の 12 個のうち 9 個

| 名前 | Yokan | 備考 |
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

## `textwrap` — Python の 6 個のうち 2 個

| 名前 | Yokan | 備考 |
|---|---|---|
| `TextWrapper` | — | |
| `dedent` | ✓ |  |
| `fill` | — | |
| `indent` | ✓ |  |
| `shorten` | — | |
| `wrap` | — | |

## `bisect` — Python の 6 個のうち 3 個

| 名前 | Yokan | 備考 |
|---|---|---|
| `bisect` | ✓ |  |
| `bisect_left` | ✓ |  |
| `bisect_right` | ✓ |  |
| `insort` | — | |
| `insort_left` | — | |
| `insort_right` | — | |

## `heapq` — Python の 13 個のうち 2 個

| 名前 | Yokan | 備考 |
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

## `re` — Python の 33 個のうち 7 個

| 名前 | Yokan | 備考 |
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

## `datetime` — Python の 10 個のうち 3 個

| 名前 | Yokan | 備考 |
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

## `collections` — Python の 10 個のうち 1 個

| 名前 | Yokan | 備考 |
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

## `itertools` — Python の 20 個のうち 6 個

| 名前 | Yokan | 備考 |
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

