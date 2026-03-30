## Player functionality

Public and visible on landing page. Only supports one live game at a time so no game selector necessary.

### Blinds

Most important page

- Show current blinds
- Show next blinds
- Show time until next blinds / time until next break

### Game rules

Not as important, parts can be shown in the main page but shouldn't all be there to avoid cluttering main page. Rules and similar should be reachable with a click

- Show possible poker hands in order
- Show what chip color has what value
- Show other rules
  - what order to play in (under the gun, ..., dealer, small blind, big blind, then starting with small blind)
  - minimum raise limits
  - ...

## Admin functionality

Different path, can be guessable, does not need to be secure but ideally no link from main page

### Before the game

- Enter number of tables
- Enter number of players
- Enter available chips (list total amounts of each color)
- Set schedule for the event, including breaks (e.g. 4h30min total, 3x15 min play, then 15min break + merge tables)

### At start of game

- Suggest how many chips of each color each player has at the start and what value they have each

### During the game

- Reenter / Update any previous values and adjust schedule + timer (e.g. pause, resume, set manually to another value)
- Speed up / slow down blind progression manually with simple buttons.

### At all times

- Change / manipulate any state of the game, including resetting things, restarting, starting again from a different point, ...

## Potential future functionality

- seating plan + suggested merges (not now, not important and only useful if it's not a hassle to use)
