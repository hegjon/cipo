# Juice me

## State dir

The changes are written to files append-only log-file style, each transaction have their own file.

These files can be used for debugging and are used by the program on startup.

### Structure
<timestamp> <remaining-watt-hours>

### Example 2

```
2022-04-15T20:38:32.417830635Z +0.039
2022-04-15T20:38:37.526962839Z +0.019
2022-04-15T20:38:42.607861054Z +0.000
2022-04-15T20:38:47.674914008Z -0.020
```