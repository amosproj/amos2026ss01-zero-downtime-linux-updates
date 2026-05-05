# Dev Environment

This repository provides a DevContainer definition for a standardized environment.

The recommended setup for development on this project therefore is ```Microsoft Visual Studio Code``` with the ```Dev Containers``` extension.

## Setup

To get started, follow the [Microsoft guide](https://code.visualstudio.com/docs/devcontainers/containers#_installation) on setting up Dev Containers.
Also setup [Git credential sharing](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials) into the container.

Now clone the repository and open it in VS Code. You should be automatically prompted by a notification in the bottom right to ```Reopen in Container```.
After doing so and waiting for it to finish (building the container the first time can take a few minutes), the whole environment should be set up and ready to go.

## Caveats

Dev Containers should be viewed as transient and read-only. Do not make changes to your running container, always change or extend the definition and rebuild to ensure replicability. When changing any file in the ```.devcontainer/``` folder also change the ```devcontainer.json``` file, so people get prompted to rebuild their containers when pulling.