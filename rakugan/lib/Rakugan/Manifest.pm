package Rakugan::Manifest;
# Generated from crates/yokan-stdlib/stdlib.toml by tools/gen_capi.pl.
# Do not edit by hand; edit the manifest and run the generator.
#
# The framework's own standard library as data: one row per function
# that both runs land on. The translator reads this to write the
# `.pix`; Rakugan::Stdlib reads the same rows to make the call through
# the C face. Plain data, and nothing newer than perl 5.34 is written
# here.
use strict;
use warnings;

our @ROWS = (
    { id => 1, module => 'fs', name => 'read_text', class => 'Fs', fn => 'readText', ret => 'str', ret_ty => 'String', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 2, module => 'fs', name => 'write_text', class => 'Fs', fn => 'writeText', ret => 'int', ret_ty => 'Int', sig => 'path: String, text: String', kinds => ['str', 'str'], try => 1 },
    { id => 3, module => 'fs', name => 'exists', class => 'Fs', fn => 'exists', ret => 'bool', ret_ty => 'Bool', sig => 'path: String', kinds => ['str'] },
    { id => 4, module => 'fs', name => 'read_text_or', class => 'Fs', fn => 'readTextOr', ret => 'str', ret_ty => 'String', sig => 'path: String, default: String', kinds => ['str', 'str'] },
    { id => 5, module => 'fs', name => 'list_dir', class => 'Fs', fn => 'listDir', ret => 'strs', ret_ty => 'List<String>', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 6, module => 'fs', name => 'append_text', class => 'Fs', fn => 'appendText', ret => 'int', ret_ty => 'Int', sig => 'path: String, text: String', kinds => ['str', 'str'], try => 1 },
    { id => 7, module => 'fs', name => 'remove', class => 'Fs', fn => 'remove', ret => 'int', ret_ty => 'Int', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 8, module => 'fs', name => 'make_dir', class => 'Fs', fn => 'makeDir', ret => 'int', ret_ty => 'Int', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 9, module => 'fs', name => 'app_dir', class => 'Fs', fn => 'appDir', ret => 'str', ret_ty => 'String', sig => 'name: String', kinds => ['str'], try => 1 },
    { id => 10, module => 'fs', name => 'open_dialog', class => 'Fs', fn => 'openDialog', ret => 'str', ret_ty => 'String', sig => 'title: String', kinds => ['str'] },
    { id => 11, module => 'fs', name => 'save_dialog', class => 'Fs', fn => 'saveDialog', ret => 'str', ret_ty => 'String', sig => 'name: String', kinds => ['str'] },
    { id => 12, module => 'fs', name => 'size', class => 'Fs', fn => 'size', ret => 'int', ret_ty => 'Int', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 13, module => 'fs', name => 'modified_ms', class => 'Fs', fn => 'modifiedMs', ret => 'int', ret_ty => 'Int', sig => 'path: String', kinds => ['str'], try => 1 },
    { id => 14, module => 'fs', name => 'is_dir', class => 'Fs', fn => 'isDir', ret => 'bool', ret_ty => 'Bool', sig => 'path: String', kinds => ['str'] },
    { id => 15, module => 'fs', name => 'read_text_from', class => 'Fs', fn => 'readTextFrom', ret => 'str', ret_ty => 'String', sig => 'path: String, offset: Int', kinds => ['str', 'int'], try => 1 },
    { id => 16, module => 'sqlite', name => 'exec', class => 'Sqlite', fn => 'exec', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String', kinds => ['str', 'str'], try => 1 },
    { id => 17, module => 'sqlite', name => 'query_text', class => 'Sqlite', fn => 'queryText', ret => 'strs', ret_ty => 'List<String>', sig => 'path: String, sql: String', kinds => ['str', 'str'], try => 1 },
    { id => 18, module => 'sqlite', name => 'query_int', class => 'Sqlite', fn => 'queryInt', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String', kinds => ['str', 'str'], try => 1 },
    { id => 19, module => 'sqlite', name => 'query_int_or', class => 'Sqlite', fn => 'queryIntOr', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String, default: Int', kinds => ['str', 'str', 'int'] },
    { id => 20, module => 'sqlite', name => 'query_text_or', class => 'Sqlite', fn => 'queryTextOr', ret => 'strs', ret_ty => 'List<String>', sig => 'path: String, sql: String', kinds => ['str', 'str'] },
    { id => 21, module => 'sqlite', name => 'exec', class => 'Sqlite', fn => 'execWith', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'], try => 1 },
    { id => 22, module => 'sqlite', name => 'query_text', class => 'Sqlite', fn => 'queryTextWith', ret => 'strs', ret_ty => 'List<String>', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'], try => 1 },
    { id => 23, module => 'sqlite', name => 'query_int', class => 'Sqlite', fn => 'queryIntWith', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'], try => 1 },
    { id => 24, module => 'sqlite', name => 'query_int_or', class => 'Sqlite', fn => 'queryIntOrWith', ret => 'int', ret_ty => 'Int', sig => 'path: String, sql: String, default: Int, params: List<String>', kinds => ['str', 'str', 'int', 'list'] },
    { id => 25, module => 'sqlite', name => 'query_text_or', class => 'Sqlite', fn => 'queryTextOrWith', ret => 'strs', ret_ty => 'List<String>', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'] },
    { id => 26, module => 'sqlite', name => 'query_rows', class => 'Sqlite', fn => 'queryRows', ret => 'rows', ret_ty => 'List<List<String>>', sig => 'path: String, sql: String', kinds => ['str', 'str'], try => 1 },
    { id => 27, module => 'sqlite', name => 'query_rows', class => 'Sqlite', fn => 'queryRowsWith', ret => 'rows', ret_ty => 'List<List<String>>', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'], try => 1 },
    { id => 28, module => 'sqlite', name => 'query_rows_or', class => 'Sqlite', fn => 'queryRowsOr', ret => 'rows', ret_ty => 'List<List<String>>', sig => 'path: String, sql: String', kinds => ['str', 'str'] },
    { id => 29, module => 'sqlite', name => 'query_rows_or', class => 'Sqlite', fn => 'queryRowsOrWith', ret => 'rows', ret_ty => 'List<List<String>>', sig => 'path: String, sql: String, params: List<String>', kinds => ['str', 'str', 'list'] },
    { id => 30, module => 'clipboard', name => 'set_text', class => 'Clipboard', fn => 'setText', ret => 'int', ret_ty => 'Int', sig => 'text: String', kinds => ['str'] },
    { id => 31, module => 'clipboard', name => 'get_text', class => 'Clipboard', fn => 'getText', ret => 'str', ret_ty => 'String', sig => '', kinds => [] },
    { id => 32, module => 'keys', name => 'down', class => 'Keys', fn => 'down', ret => 'bool', ret_ty => 'Bool', sig => 'key: String', kinds => ['str'] },
    { id => 33, module => 'keys', name => 'pressed', class => 'Keys', fn => 'pressed', ret => 'bool', ret_ty => 'Bool', sig => 'key: String', kinds => ['str'] },
    { id => 34, module => 'keys', name => 'released', class => 'Keys', fn => 'released', ret => 'bool', ret_ty => 'Bool', sig => 'key: String', kinds => ['str'] },
    { id => 35, module => 'audio', name => 'play', class => 'Audio', fn => 'play', ret => 'int', ret_ty => 'Int', sig => 'path: String', kinds => ['str'] },
    { id => 36, module => 'audio', name => 'play', class => 'Audio', fn => 'playAt', ret => 'int', ret_ty => 'Int', sig => 'path: String, volume: Float', kinds => ['str', 'num'] },
    { id => 37, module => 'audio', name => 'stop', class => 'Audio', fn => 'stop', ret => 'int', ret_ty => 'Int', sig => '', kinds => [] },
    { id => 38, module => 'notify', name => 'send', class => 'Notify', fn => 'send', ret => 'void', ret_ty => '', sig => 'title: String, body: String', kinds => ['str', 'str'] },
    { id => 39, module => 'http', name => 'get_text', class => 'Http', fn => 'getText', ret => 'str', ret_ty => 'String', sig => 'url: String', kinds => ['str'], try => 1 },
    { id => 40, module => 'http', name => 'get_text_or', class => 'Http', fn => 'getTextOr', ret => 'str', ret_ty => 'String', sig => 'url: String, default: String', kinds => ['str', 'str'] },
    { id => 41, module => 'http', name => 'get_text', class => 'Http', fn => 'getTextTimeout', ret => 'str', ret_ty => 'String', sig => 'url: String, timeoutMs: Int', kinds => ['str', 'int'], try => 1 },
    { id => 42, module => 'http', name => 'post_text', class => 'Http', fn => 'postText', ret => 'str', ret_ty => 'String', sig => 'url: String, body: String', kinds => ['str', 'str'], try => 1 },
    { id => 43, module => 'http', name => 'post_text', class => 'Http', fn => 'postTextAs', ret => 'str', ret_ty => 'String', sig => 'url: String, body: String, contentType: String', kinds => ['str', 'str', 'str'], try => 1 },
    { id => 44, module => 'http', name => 'post_text_or', class => 'Http', fn => 'postTextOr', ret => 'str', ret_ty => 'String', sig => 'url: String, body: String, default: String', kinds => ['str', 'str', 'str'] },
    { id => 45, module => 'http', name => 'status', class => 'Http', fn => 'status', ret => 'int', ret_ty => 'Int', sig => 'url: String', kinds => ['str'] },
    { id => 46, module => 'jsondoc', name => 'get_text', class => 'Jsondoc', fn => 'getText', ret => 'str', ret_ty => 'String', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 47, module => 'jsondoc', name => 'get_int', class => 'Jsondoc', fn => 'getInt', ret => 'int', ret_ty => 'Int', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 48, module => 'jsondoc', name => 'get_float', class => 'Jsondoc', fn => 'getFloat', ret => 'num', ret_ty => 'Float', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 49, module => 'jsondoc', name => 'get_bool', class => 'Jsondoc', fn => 'getBool', ret => 'bool', ret_ty => 'Bool', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 50, module => 'jsondoc', name => 'length', class => 'Jsondoc', fn => 'length', ret => 'int', ret_ty => 'Int', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 51, module => 'jsondoc', name => 'has', class => 'Jsondoc', fn => 'has', ret => 'bool', ret_ty => 'Bool', sig => 'src: String, path: String', kinds => ['str', 'str'], pure => 1, try => 1 },
    { id => 52, module => 'jsondoc', name => 'get_texts', class => 'Jsondoc', fn => 'getTexts', ret => 'strs', ret_ty => 'List<String>', sig => 'src: String, paths: List<String>, default: String', kinds => ['str', 'list', 'str'], pure => 1, try => 1 },
    { id => 53, module => 'strings', name => 'to_int', class => 'Strings', fn => 'toInt', ret => 'int', ret_ty => 'Int', sig => 's: String, default: Int', kinds => ['str', 'int'], pure => 1 },
    { id => 54, module => 'strings', name => 'to_float', class => 'Strings', fn => 'toFloat', ret => 'num', ret_ty => 'Float', sig => 's: String, default: Float', kinds => ['str', 'num'], pure => 1 },
    { id => 55, module => 'clock', name => 'format_ms', class => 'Clock', fn => 'formatMs', ret => 'str', ret_ty => 'String', sig => 'ms: Int, fmt: String', kinds => ['int', 'str'], try => 1 },
    { id => 56, module => 'clock', name => 'format_local_ms', class => 'Clock', fn => 'formatLocalMs', ret => 'str', ret_ty => 'String', sig => 'ms: Int, fmt: String', kinds => ['int', 'str'], try => 1 },
    { id => 57, module => 'clock', name => 'local_offset_minutes', class => 'Clock', fn => 'localOffsetMinutes', ret => 'int', ret_ty => 'Int', sig => 'ms: Int', kinds => ['int'], try => 1 },
);

# By the name an app writes and how many values it wrote: two rows of
# one name are two arities, the way `sqlite_exec` takes a statement
# with or without values to bind.
our %BY_CALL = map { ("$_->{module}_$_->{name}/" . scalar @{ $_->{kinds} }) => $_ } @ROWS;
our %NAMES = map { ("$_->{module}_$_->{name}" => 1) } @ROWS;

1;
