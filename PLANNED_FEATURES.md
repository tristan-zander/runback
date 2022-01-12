# Rematch - Discord Tournament Bot (name pending)

## Features
The bot will be split into the following 3 categories:
1. Matchmaking service
1. League service (with rankings) <!-- It'll be like a "Cup" or "League", where multiple games are played across a long period of time. -->
1. Bracket service

### Matchmaking
The matchmaking service enables users to look for games with other people in a Discord server.
In Matchmaking, there are no intended limits how who can play and how many sets are to be played.
Using *Rematch* in place of a server's LFG system will provide the following benefits:

1. Users can choose to only be pinged for games during a time that they like, and they will receive a Direct Message whenever a match is ready.
1. Channels won't be clogged with multiple ongoing games, as the bot will create a temporary, private channel for that session.
1. Discovering matches will be easier than ever. Filter by game, number of players, gamemode, and even (potentially) cross-server matches.

#### Feature List
- `/mm` brings up a prompt with settings and options that you can pick while you're searching.
  - Advertises in either a specified channel or DMs that you're entering LFG
  - Alerts the user via ping or DM that a match is available
  - Alternatively, click on a user's profile and select `Ask for Games` to start a matchmaking session
  - Default time (15min) before LFG ends
- Users can pick from the available list of players who they wish to play against
- When a match is found, setup a channel for users to communicate through
  - `/mm visibility {public/private}` admins and/or users can decide whether they want their chat to be public/private
  - Default time (30min) before ending that session unless extended (prevents orphaned channel buildup)
  - `/mm end-match` to end a match/set/game 
  - Alternatively, click on a user's profile and select `End Match` to end the ongoing match
- `/mm quit` to quit matchmaking
- `/mm options` bring up a prompt that lets users decide their preferences for alerts, etc
- `/mm show-games` show available games to join (ephemeral message)
- `/mm report-abuse` report a player for misconduct (cheating, bullying, etc)

