# Runback - Matchmaking and Tournament Bot
A discord bot for holding tournament brackets, general matchmaking, and hosting ranked leagues.

## Development
In order to setup the development environment, make sure that you have Docker and docker-compose installed on your system.
To test your development with live reloading, run the following command:
```shell
# Start service and all dependencies.
docker-compose -f docker-compose.development.yml up discord-bot
```
The development environment will have access to your entire working directory through a Docker volume. Production deployments will only be able to access their subfolder and any dependencies, so make sure that you also test any code modifications under the Production environment as well.
