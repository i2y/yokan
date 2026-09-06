package Rakugan::Stdlib;
# Generated from crates/yokan-stdlib/stdlib.toml by tools/gen_capi.pl.
# Do not edit by hand; edit the manifest and run the generator.
#
# The framework's own standard library, as one Perl sub per row. Each
# one pushes its arguments through the C face and reads the answer back
# — the same Rust the compiled run links through the binding door, so
# the two runs cannot be answering different libraries.
#
# The rows themselves are Rakugan::Manifest's, which the translator
# reads too.
use v5.40;
use Rakugan::Door;
use Rakugan::Manifest;

our @ROWS = @Rakugan::Manifest::ROWS;
our %BY_CALL = %Rakugan::Manifest::BY_CALL;
our %NAMES = %Rakugan::Manifest::NAMES;

# One call: the arguments in, the row's number, the answer out.
sub call ($row, @args) {
    Rakugan::Door::std_reset();
    for my $i (0 .. $#args) {
        my $kind = $row->{kinds}[$i];
        if    ($kind eq 'str')  { Rakugan::Door::std_arg_str("$args[$i]") }
        elsif ($kind eq 'int')  { Rakugan::Door::std_arg_int(int $args[$i]) }
        elsif ($kind eq 'num')  { Rakugan::Door::std_arg_num(0 + $args[$i]) }
        else {
            Rakugan::Door::std_arg_list_begin();
            Rakugan::Door::std_arg_str("$_") for @{ $args[$i] };
            Rakugan::Door::std_arg_list_end();
        }
    }
    my $n = Rakugan::Door::std_call($row->{id});
    return $n                                 if $row->{ret} eq 'int';
    return $n != 0 ? true : false             if $row->{ret} eq 'bool';
    return Rakugan::Door::std_answer_num()    if $row->{ret} eq 'num';
    return cell(0, 0)                         if $row->{ret} eq 'str';
    # A list answer comes back as a list, which is what perl hands a
    # caller; a query's rows come back as a list of rows.
    return map { cell($_, 0) } 0 .. Rakugan::Door::std_rows() - 1 if $row->{ret} eq 'strs';
    return map { my $r = $_; [map { cell($r, $_) } 0 .. Rakugan::Door::std_cells($r) - 1] }
           0 .. Rakugan::Door::std_rows() - 1 if $row->{ret} eq 'rows';
    return;
}

sub cell ($row, $col) {
    Rakugan::Door::std_pick($row, $col);
    return Rakugan::Door::answer_text();
}


# `fs.read_text` in the manifest: (path).
sub fs_read_text { Rakugan::Stdlib::dispatch('fs_read_text', @_) }

# `fs.write_text` in the manifest: (path, text).
sub fs_write_text { Rakugan::Stdlib::dispatch('fs_write_text', @_) }

# `fs.exists` in the manifest: (path).
sub fs_exists { Rakugan::Stdlib::dispatch('fs_exists', @_) }

# `fs.read_text_or` in the manifest: (path, default).
sub fs_read_text_or { Rakugan::Stdlib::dispatch('fs_read_text_or', @_) }

# `fs.list_dir` in the manifest: (path).
sub fs_list_dir { Rakugan::Stdlib::dispatch('fs_list_dir', @_) }

# `fs.append_text` in the manifest: (path, text).
sub fs_append_text { Rakugan::Stdlib::dispatch('fs_append_text', @_) }

# `fs.remove` in the manifest: (path).
sub fs_remove { Rakugan::Stdlib::dispatch('fs_remove', @_) }

# `fs.make_dir` in the manifest: (path).
sub fs_make_dir { Rakugan::Stdlib::dispatch('fs_make_dir', @_) }

# `fs.app_dir` in the manifest: (name).
sub fs_app_dir { Rakugan::Stdlib::dispatch('fs_app_dir', @_) }

# `fs.open_dialog` in the manifest: (title).
sub fs_open_dialog { Rakugan::Stdlib::dispatch('fs_open_dialog', @_) }

# `fs.save_dialog` in the manifest: (name).
sub fs_save_dialog { Rakugan::Stdlib::dispatch('fs_save_dialog', @_) }

# `sqlite.exec` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_exec { Rakugan::Stdlib::dispatch('sqlite_exec', @_) }

# `sqlite.query_text` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_query_text { Rakugan::Stdlib::dispatch('sqlite_query_text', @_) }

# `sqlite.query_int` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_query_int { Rakugan::Stdlib::dispatch('sqlite_query_int', @_) }

# `sqlite.query_int_or` in the manifest: (path, sql, default) or (path, sql, default, params).
sub sqlite_query_int_or { Rakugan::Stdlib::dispatch('sqlite_query_int_or', @_) }

# `sqlite.query_text_or` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_query_text_or { Rakugan::Stdlib::dispatch('sqlite_query_text_or', @_) }

# `sqlite.query_rows` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_query_rows { Rakugan::Stdlib::dispatch('sqlite_query_rows', @_) }

# `sqlite.query_rows_or` in the manifest: (path, sql) or (path, sql, params).
sub sqlite_query_rows_or { Rakugan::Stdlib::dispatch('sqlite_query_rows_or', @_) }

# `clipboard.set_text` in the manifest: (text).
sub clipboard_set_text { Rakugan::Stdlib::dispatch('clipboard_set_text', @_) }

# `clipboard.get_text` in the manifest: ().
sub clipboard_get_text { Rakugan::Stdlib::dispatch('clipboard_get_text', @_) }

# `keys.down` in the manifest: (key).
sub keys_down { Rakugan::Stdlib::dispatch('keys_down', @_) }

# `keys.pressed` in the manifest: (key).
sub keys_pressed { Rakugan::Stdlib::dispatch('keys_pressed', @_) }

# `keys.released` in the manifest: (key).
sub keys_released { Rakugan::Stdlib::dispatch('keys_released', @_) }

# `audio.play` in the manifest: (path) or (path, volume).
sub audio_play { Rakugan::Stdlib::dispatch('audio_play', @_) }

# `audio.stop` in the manifest: ().
sub audio_stop { Rakugan::Stdlib::dispatch('audio_stop', @_) }

# `notify.send` in the manifest: (title, body).
sub notify_send { Rakugan::Stdlib::dispatch('notify_send', @_) }

# `http.get_text` in the manifest: (url) or (url, timeoutMs).
sub http_get_text { Rakugan::Stdlib::dispatch('http_get_text', @_) }

# `http.get_text_or` in the manifest: (url, default).
sub http_get_text_or { Rakugan::Stdlib::dispatch('http_get_text_or', @_) }

# `http.post_text` in the manifest: (url, body) or (url, body, contentType).
sub http_post_text { Rakugan::Stdlib::dispatch('http_post_text', @_) }

# `http.post_text_or` in the manifest: (url, body, default).
sub http_post_text_or { Rakugan::Stdlib::dispatch('http_post_text_or', @_) }

# `http.status` in the manifest: (url).
sub http_status { Rakugan::Stdlib::dispatch('http_status', @_) }

# `jsondoc.get_text` in the manifest: (src, path).
sub jsondoc_get_text { Rakugan::Stdlib::dispatch('jsondoc_get_text', @_) }

# `jsondoc.get_int` in the manifest: (src, path).
sub jsondoc_get_int { Rakugan::Stdlib::dispatch('jsondoc_get_int', @_) }

# `jsondoc.get_float` in the manifest: (src, path).
sub jsondoc_get_float { Rakugan::Stdlib::dispatch('jsondoc_get_float', @_) }

# `jsondoc.get_bool` in the manifest: (src, path).
sub jsondoc_get_bool { Rakugan::Stdlib::dispatch('jsondoc_get_bool', @_) }

# `jsondoc.length` in the manifest: (src, path).
sub jsondoc_length { Rakugan::Stdlib::dispatch('jsondoc_length', @_) }

# `jsondoc.has` in the manifest: (src, path).
sub jsondoc_has { Rakugan::Stdlib::dispatch('jsondoc_has', @_) }

# `strings.to_int` in the manifest: (s, default).
sub strings_to_int { Rakugan::Stdlib::dispatch('strings_to_int', @_) }

# `strings.to_float` in the manifest: (s, default).
sub strings_to_float { Rakugan::Stdlib::dispatch('strings_to_float', @_) }

# `clock.format_ms` in the manifest: (ms, fmt).
sub clock_format_ms { Rakugan::Stdlib::dispatch('clock_format_ms', @_) }

# `clock.format_local_ms` in the manifest: (ms, fmt).
sub clock_format_local_ms { Rakugan::Stdlib::dispatch('clock_format_local_ms', @_) }

# `clock.local_offset_minutes` in the manifest: (ms).
sub clock_local_offset_minutes { Rakugan::Stdlib::dispatch('clock_local_offset_minutes', @_) }

# Which row a call means: the name it was written with, and how many
# values came with it.
sub dispatch ($name, @args) {
    my $row = $BY_CALL{"$name/" . scalar @args};
    unless ($row) {
        my @arities;
        for my $r (@ROWS) {
            push @arities, scalar @{ $r->{kinds} } if "$r->{module}_$r->{name}" eq $name;
        }
        die "$name takes " . join(' or ', sort @arities) . " values, and got " . scalar(@args) . "\n";
    }
    return call($row, @args);
}

our @EXPORT = sort keys %NAMES;

1;
