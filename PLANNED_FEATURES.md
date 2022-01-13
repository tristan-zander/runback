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

### League
The league service will enable admins to setup leagues that players can participate in for points. 
Players will play their matches over a set period of time, playing their matches as directed.
Alternatively, players can decide to start matches with anyone that they like at any time, working more like a ranked ladder.
At the end of a league, points are tallyed and a leaderboard is posted in the relevant Discord channel.

League matches will follow a format similar to the matchmaking service.
Players will either be called upon to start their match or initiate it themselves in an allotted timeframe.
Once players start their match, an admin may be notified and a channel automatically generated for the players to use.

Potentially, I see alot of potential for integration with this feature.
Twitch integration can be used to advertise players or admins streaming their matches.
Another service may offer betting fake points and offer rewards in exchange.

#### Feature List
- All admins will manage their leagues through a context menu returned by a Discord command or through a website interface
- `/league create` brings up a context menu that allows admins to setup a league 
- `/league options` brings up an admin menu for current league settings
  - Set relevant roles for admins, players, League Organizers, etc.
  - Set relevant channels for matches and reporting
  - Set start and end date
  - Set player rights
- `/league invite {DiscordUser}` invite a player to join in the league
- `/league team ...` Commands related to team leagues
  - `create {Name}`
  - `invite {DiscordUser}` invite a player to your team
  - `options` show a context menu with team options
    - Changing the name of the group
    - Removing players
    - Reassigning the group leader
- `/league match ...` commands related to matches
  - `list {Mine:default/Team/Player/All}` show scheduled matches (filter by players/team)
  - `start {Player/Team}` start a match with a player or team
  - `report {Score} {Match}` report the score for a match
