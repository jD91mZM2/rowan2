An experimental rewrite of [rowan](https://github.com/rust-analyzer/rowan)
focusing on arena-based trees and mutability.

**This is just a test.** Parts of this should probably be extracted upstream.

## Arena

Arena-based trees are awesome.

 - I measure a 1.5x speedup (from 10,091 to a whooping 6,533 ns/iter) in [rnix](https://gitlab.com/jD91mZM2/rnix).
 - Less things need to be mutated or reference counted - only the arena.

## Mutability

Unless you want to do like in a very old version of rnix where I let the user
pass the arena around EVERYWHERE, arenas should be reference counted. This
unfortunately means you need interior mutability to modify it, but luckily it's
only the arena and nothing else.

Mutable trees discard all range data, but it should be possible to make a
function that re-calculates that and then makes the tree immutable again.

rowan2 uses reference counters and refcells by default, but supplies the
`thread` conditional compilation flag which uses atomic reference counters and
mutexes
