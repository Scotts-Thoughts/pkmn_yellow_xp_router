# Pokemon Solo Challenge Router

This is a tool that is intended to allow easy experience routing of Solo Pokemon playthroughs of Pokemon, generations 1-3.
It keeps track of the experience and stat experience gained over the course of a run, as well as your moveset.
It also tracks your inventory, and your money.
While it will allow you to route any item usage, and any purchases, if it detects an invalid game state, it will flag events (e.g. using items you don't have, using money you don't have).
It is not intended to route the game for you, merely allow you to quickly test out ideas without requiring you replay large chunks of the game.
It assumes that a single pokemon has been injected to Pokemon Yellow as your starter, via the Universal Pokemon Randomizer, and that the pokemon will be receiving 100% of the experience over the course of the run.

**NOTE:** This is a solo project done in my free time, so there are definitely some rough edges. I have manually verified the accuracy with several routes myself, but I cannot guarantee 100% accuracy. If you notice any discrepancies, let me know!
I recommend playing through the route once after mapping it out with this tool (or even playing it alongside the tool as you map it out) to guarantee its accuracy.

## Move stats in route files

Routes recorded with move stats (Rust router, late 2026 and later) keep a `move_stats` object on each trainer battle and at the top of the file.
Older routes load and save exactly as before. **An older router (an older Rust build, the Python app or an old iOS build) loads a new route fine but drops `move_stats` if it saves the file again.** The automatic backup keeps the previous file.
Details: `docs/rust_port/recording/move_stats.md`.

## Project status

I have the last major update of this project. If you find any bugs, I will try to fix them, but this project will receive no major updates moving forward.

## Final Notes

This is largely just a personal project that I thought some other people might want to play around with.
If you find any issues feel free to contact me, but I make no promises that this project will be maintainned in the future.
Use/reuse/steal any code, as you wish!
Or maybe don't, it's kinda hacky :)
