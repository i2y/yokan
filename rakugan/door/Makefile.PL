use ExtUtils::MakeMaker;
# The engine's C face is built into the shared cargo target like every
# crate here; bin/rakugan passes the directory. The dylib carries an
# absolute install name, so the rpath is belt and braces.
my $lib = $ENV{PIXIE_CAPI_DIR} // "$ENV{HOME}/.cache/pixie/target/release";
WriteMakefile(
    NAME        => 'Rakugan::Door',
    VERSION     => '0.1.0',
    XS          => { 'Door.xs' => 'Door.c' },
    dynamic_lib => { OTHERLDFLAGS => "-L$lib -lpixie_capi -Wl,-rpath,$lib" },
);
