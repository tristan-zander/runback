# Runback - Matchmaking and Tournament Bot
A discord bot for holding tournament brackets, general matchmaking, and hosting ranked leagues.

[![CircleCI](https://circleci.com/gh/tristan-zander/runback.svg?style=svg)](https://circleci.com/gh/tristan-zander/runback)

## Development
In order to setup the development environment, make sure that you have Docker and docker-compose installed on your system.
To test your development with live reloading, run the following command:
```shell
# Start service and all dependencies.
docker-compose -f docker-compose.development.yml up discord-bot
```
The development environment will have access to your entire working directory through a Docker volume. Production deployments will only be able to access their subfolder and any dependencies, so make sure that you also test any code modifications under the Production environment as well.

### Discord Bot Setup
The following scopes need to be given to your development bot:
- `bot`
- `application.commands`

The following bot permissions need to be added to your development bot:
- Read Messages/View Channels
- Send Messages
- Create Public Threads
- Create Private Threads
- Send Messages in Threads
- Manage Threads
- Embed Links
- Read Message History
- Add Reactions
- Use Slash Commands

Additionally, you need to grant your bot Privileged Gateway Intents for `GUILD_MEMBERS`, which is available at your bot's settings.
