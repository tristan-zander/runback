# Runback - Matchmaking and Tournament Bot
A discord bot for holding tournament brackets, general matchmaking, and hosting ranked leagues.

## Development
In order to setup the development environment, make sure that you have Docker and docker-compose installed on your system.
To test your development with live reloading, run the following command:
```shell
# Start service and all dependencies.
docker-compose -f docker-compose.development.yml up [service_name] [additional_service_name...]
```
The development environment will have access to your entire working directory through a Docker volume. Production deployments will only be able to access their subfolder and any dependencies, so make sure that you also test your code with the standard docker-compose file.

Please note that some services will not work without some configuration. For instance, keycloak will require you to create a realm and service for the `matchmaking` service. Please message me at `Galestrike#8814` on Discord or submit an issue if there's any undocumented configuration steps.
