# Contributing Guidelines
Thank you for your interest in contributing to Runback. Community feedback and contributtions are essential for Runback to be both featureful and dependable.

## Commit Guidelines
Releases are created using [semantic release](https://semantic-release.gitbook.io/semantic-release/). Please ensure that the final commit in your pull request adheres to that standard.

## Issue Reporting
Please use Runback's [issue tracker](https://github.com/tristan-zander/runback/issues) to report new issues.

## Setup
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
