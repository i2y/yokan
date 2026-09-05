# The one file an app requires. Which door answers `wakakusa/door` is
# decided by the load path: `door/cruby` while the app is being written,
# `door/spinel` when it is compiled. Everything else here is the same
# Ruby in both runs, which is what makes comparing the two worth doing.
require "wakakusa/keys"
require "wakakusa/door"
require "wakakusa/elements"
require "wakakusa/runtime"
