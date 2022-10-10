if [ -z $1 ]
then
    echo "build_docker.sh requires a version number to be passed"
    exit 1
fi

VERSION=$1

cargo install --root app --bin discord-bot --path . --all-features

docker build -t discord-bot:${VERSION} .
docker tag discord-bot:${VERSION} registry.digitalocean.com/runback/discord-bot
