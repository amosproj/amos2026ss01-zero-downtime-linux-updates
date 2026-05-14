# Target Architecture

This document reflects the current target architecture

## 1. Goal

- A user operates the cloud via API/UI.
- The cloud persists current state of all Edge IPCs in PostgreSQL.
- The edge IPCs each run an `Orchestrator`.
- The Download Manager checks whether OS/apps are up to date, then informs Orchestrator.
- Update artifacts are pulled from a product source (GHCR).

## 2. System Architecture

```mermaid
flowchart LR
    %% Actors
    User[User]

    %% Cloud side
    subgraph Cloud[Cloud]
        API[Cloud API]
        DB[(PostgreSQL)]
        API <--> DB
    end

    %% Edge side
    subgraph Edge["Edge IPCs (1..n)"]
        Orchestrator[Orchestrator]
        DM[Download Manager]
        SEC{Security Check}
        BOOTC[bootc]
        PODMAN[Podman]

        Orchestrator -->|Updated os/app state for DB| DM
        Orchestrator -->|Trigger OS update| SEC
        Orchestrator -->|Trigger app update| SEC
        SEC -->|Signature verified| BOOTC
        SEC -->|Signature verified| PODMAN
    end

    %% External source
    Product["GitHub (GHCR)"]

    %% Interactions
    User -->|Management/API calls| API

    
    
    API <-->|OS & app state| DM
    DM -->|Update needed| Orchestrator
    BOOTC -->|Download + stage OS image| Product
    PODMAN -->|Pull app image| Product

    classDef cloud fill:#1f3b64,color:#fff,stroke:#0f2038,stroke-width:1px;
    classDef edge fill:#1f5f3a,color:#fff,stroke:#0f3320,stroke-width:1px;
    classDef ext fill:#5b2b6f,color:#fff,stroke:#361944,stroke-width:1px;

    class API,DB cloud;
    class Orchestrator,DM,BOOTC,PODMAN,SEC edge;
    class Product ext;
    style Cloud fill:#eef9ff,stroke:#4aa3df,stroke-width:2px,color:#0b3557

```

## 3. Main Control Loop (Concept)

1. `Download Manager` polls `Cloud`.
2. Cloud returns desired state for OS and applications.
3. If update is needed:
   - OS path via `bootc`
   - App path via `Podman`
4. `Download Manager` reports update result/status to cloud.
5. Cloud stores state in PostgreSQL.
