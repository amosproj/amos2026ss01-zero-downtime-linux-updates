# Software Architecture: Zero Downtime Linux Updates

## 1. Executive Summary
This document outlines the initial architecture for the Zero-Downtime Update system deployed on an Edge IPC. The architecture focuses on NIS2 compliance, hardware-backed security (TPM), and a robust pull-based update mechanism designed for industrial environments with restrictive network policies.

## 2. System Overview
The architecture is based on a distributed edge-cloud model designed to maintain operational continuity ("Zero Downtime") while enforcing a strict security posture.

```mermaid
graph TD
    %% definition cloud
    subgraph Weber_Cloud ["Weber Cloud (Backend)"]
        DM[Device Management]
        API1[API 1: Device Communication<br/><i>Existing</i>]
        API2[API 2: Management API<br/><i>Second Priority</i>]
        UI[Management UI]
        
        DM --- API1
        DM --- API2
        UI --- API2
    end

    %% external internet for updates
    subgraph Public_Internet ["Public Internet"]
        OS_Source[OS Update Sources]
        App_Source[Application Update Sources]
    end

    %% customer network
    subgraph Customer_Site ["Customer Network"]
        subgraph Edge_IPC ["Edge IPC"]
            direction TB
              subgraph Docker
                AM[App Management Service<br/><i>Second Priority</i>]
              end
              OUS[OS Update Service<br/><i>First Priority</i>]
        end
    end

    %% Connections and Datatransfers
    DM <-- "Fetches Updates" --> OS_Source
    DM <-- "Fetches Images" --> App_Source

    Edge_IPC <-- "Outbound Pull<br/>(Port 443 / 53)" --> API1
    
    %% legend
    classDef firstPrio fill:#272f7a,stroke:#333,stroke-width:2px;
    classDef secondPrio fill:#3498db,stroke:#333,stroke-width:2px;
    class OUS firstPrio;
    class API2,AM secondPrio;
