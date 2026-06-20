# Process logger for Windows

Logs command line of all started processes to a file `process-start-log.txt` next to the
executable.

Lines can be filtered with `--include` and `--exclude`.  
If `--include` is given, everything is excluded except what matches `--include`. If `--exclude` is also given, matching
lines are excluded even if they match the `--include`. Multiple includes/excludes are possible.