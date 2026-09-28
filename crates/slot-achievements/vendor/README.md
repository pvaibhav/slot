The rcheevos achievement evaluator is vendored unmodified from
https://github.com/RetroAchievements/rcheevos at
`40d916de00fe757bab40fb4db41a7912193a48e3` (12.2.1), the revision used by NextUI.
Only the public headers, runtime, and required utility/MD5 sources are included.
Its MIT license is included here and in `licenses/rcheevos-MIT.txt` for device bundles.

`build.rs` builds these sources locally; a build never downloads C code or requires bindgen.
