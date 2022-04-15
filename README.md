# Juice me

## State dir

The changes are written to files append-only log-file style, each transaction have their own file.

These files can be used for debugging and are used by the program on startup.

### Structure
<timestamp> <command>

### Example
File 35e16cf0ffd27a0931257012f60a941a324c8a0ab502fbc802cfaaab16b37981.log

```
1234 open
1240 0.1 Wh
1300 0.2 Wh
2000 0.3 Wh
3002 0.4 Wh
3003 close
```

### Example 2

```
1234 10.000
1240 9.98
1300 2.2
2000 1.3
3002 0.4
3003 -0.001
```