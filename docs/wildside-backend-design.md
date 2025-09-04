# Wildside Backend: Functional Design Specification

## 1. Introduction

This document provides a functional design and implementation plan for the Wildside backend service. It is intended for the engineering team responsible for building, deploying, and maintaining the application. The document details the system's architecture, the responsibilities of each component, and a series of actionable tasks to guide development from the current state to the complete MVP.

The backend is a monolithic Rust application built with Actix Web, designed to be deployed as a containerised service on Kubernetes. It provides a RESTful API and a WebSocket interface for real-time communication, with computationally intensive tasks offloaded to a separate pool of background workers.

## 2. System Architecture

The system is composed of a primary API server, a set of background workers, a PostgreSQL database with a dedicated tile server, a Redis cache, and an observability stack. All components are designed to run within a Kubernetes cluster and be managed via a GitOps workflow.

```
graph TD
    subgraph "Internet"
        User[User via PWA]
    end

    subgraph "Kubernetes Cluster (wildside)"
        Ingress[Traefik Ingress]

        subgraph "Wildside Backend"
            style "Wildside Backend" fill:#f9f,stroke:#333,stroke-width:2px
            API[Actix Web API/WS Server]
            Worker[Background Worker Pool]
        end
        
        subgraph "Tile Service"
            style "Tile Service" fill:#e9f,stroke:#333,stroke-width:2px
            Martin[Martin Tile Server]
        end

        subgraph "Data Services"
            style "Data Services" fill:#ccf,stroke:#333,stroke-width:2px
            DB[(PostgreSQL w/ PostGIS)]
            Cache[(Redis)]
            ExternalAPI[External: Overpass API]
        end

        subgraph "Observability"
            style Observability fill:#cfc,stroke:#333,stroke-width:2px
            Prometheus --> Grafana
            API -- Metrics & Logs --> FluentBit --> Loki & Prometheus
            Worker -- Metrics & Logs --> FluentBit
            Martin -- Metrics --> Prometheus
        end
    end

    subgraph "External Dependencies"
        EngineLib[wildside-engine crate]
    end

    User -- HTTPS/WSS --> Ingress
    Ingress -- /api, /ws --> API
    Ingress -- /tiles --> Martin

    API -- CRUD Ops --> DB
    API -- Read/Write --> Cache
    API -- Enqueues Job --> Cache
    API -- Pushes Updates --> User

    Martin -- Reads Geospatial Data --> DB

    Worker -- Dequeues Job --> Cache
    Worker -- Invokes --> EngineLib
    Worker -- Writes Result --> DB
    Worker -- Notifies --> API
    Worker -- On-demand Enrichment --> ExternalAPI

    EngineLib -- Requires data from --> DB
```

## 3. Core Components & Implementation Plan

This section details each functional component of the backend, its current implementation status, and the required work to complete the MVP.

### 3.1. Web Application Server

The primary application entry point, responsible for handling all synchronous API and WebSocket traffic.

- **Technology:** Actix Web, Actix WS
    
- **Current Status:** A foundational Actix Web server exists in `backend/src/main.rs`. It is configured with basic logging (`tracing`), OpenAPI documentation (`utoipa`), and a working WebSocket endpoint (`/ws`).
    
- **Key Responsibilities:**
    
    - Expose a RESTful API for all application functionality (user management, route requests, etc.).
        
    - Manage WebSocket connections for real-time client communication.
        
    - Handle user authentication and session management.
        
    - Enqueue jobs for the background workers to process.
        
    - Serve a `/metrics` endpoint for Prometheus and a `/healthz` endpoint for Kubernetes probes.
        
- **Implementation Tasks:**
    
    - [ ] **Session Management:** Implement stateless, signed-cookie sessions. Use the `actix-session` crate with a cookie-based backend. The signing key must be loaded from an environment variable (`SESSION_KEY`).
        
    - [ ] **Observability:**
        
        - Integrate the `actix-web-prom` crate as middleware to expose default Prometheus metrics on a `/metrics` endpoint.
            
        - Implement a `/healthz` endpoint that returns a `200 OK` response.
            
        - Ensure all request handlers have `tracing` spans with a unique `request_id`.
            
    - [ ] **API Endpoints:**
        
        - Implement the full suite of user management endpoints (create, read, update) under `/api/users`.
            
        - Create a `/api/routes` endpoint to accept route generation requests. This endpoint should validate the input and enqueue a `GenerateRouteJob` (see § 3.4).
            

### 3.2. Route Generation Engine Integration

The core logic for calculating walking routes is encapsulated in the `wildside-engine` library. The backend is responsible for invoking this library and managing its execution.

- **Technology:** `wildside-engine` (Rust crate)
    
- **Current Status:** The `wildside-engine` crate exists as a separate repository. The `wildside` backend does not yet include it as a dependency.
    
- **Key Responsibilities:**
    
    - The backend must provide the engine with the necessary inputs: user preferences, geographical boundaries, and time constraints.
        
    - The backend must handle the output from the engine (a structured route) and persist it to the database.
        
    - Execution of the engine must not block the main server threads.
        
- **Implementation Tasks:**
    
    - [ ] **Dependency:** Add `wildside-engine` to `backend/Cargo.toml` as a local path dependency for development, to be replaced by a Git dependency in CI.
        
    - [ ] **Execution:** The `GenerateRouteJob` handled by the background worker (see § 3.4) will be the primary point of integration. The worker will call the main `wildside_engine::generate_route()` function.
        
    - [ ] **Data Access:** The engine requires access to POI data. The worker will pass a database connection or a pre-fetched dataset to the engine as required by its interface.
        

### 3.3. Data Persistence

All persistent application data is stored in a PostgreSQL database. For the MVP, the strategy for populating and maintaining this data is a hybrid model, combining a pre-seeded baseline with on-demand enrichment to ensure both high performance and data relevance.

- **Technology:** PostgreSQL with PostGIS extension, Diesel ORM, `r2d2` for connection pooling.
    
- **Current Status:** Diesel is integrated, and a `users` model exists. A `r2d2`-based connection pool is configured in `main.rs`.
    
- **Key Responsibilities:**
    
    - Store user data, including preferences and saved routes.
        
    - Store a performant, locally cached mirror of geospatial data (Points of Interest, road networks) for the routing engine.
        
    - Evolve the local dataset over time by enriching it with new data based on user requests.
        
    - Utilise the PostGIS extension for efficient spatial queries.
        
    - Store OSM tags and other flexible data in `JSONB` columns.
        

#### 3.3.1. Entity-Relationship Diagram

The following diagram illustrates the relationships between the core data entities.

```
erDiagram
    users {
        UUID id PK
        TIMESTAMPTZ created_at
        TIMESTAMPTZ updated_at
    }

    interest_themes {
        UUID id PK
        TEXT name UK
        TEXT description
    }

    user_interest_themes {
        UUID user_id PK, FK
        UUID theme_id PK, FK
    }

    pois {
        BIGINT id PK
        GEOGRAPHY(Point, 4326) location "GIST index"
        JSONB osm_tags "GIN index"
        TEXT narrative
        REAL popularity_score
    }

    poi_interest_themes {
        BIGINT poi_id PK, FK
        UUID theme_id PK, FK
    }

    routes {
        UUID id PK
        UUID user_id FK
        GEOMETRY(LineString, 4326) path "GIST index"
        JSONB generation_params
        TIMESTAMPTZ created_at
    }

    route_pois {
        UUID route_id PK, FK
        BIGINT poi_id PK, FK
        INTEGER "order"
    }

    users ||--o{ user_interest_themes : "has"
    interest_themes ||--o{ user_interest_themes : "is chosen in"
    pois ||--o{ poi_interest_themes : "has"
    interest_themes ||--o{ poi_interest_themes : "is assigned to"
    users ||--o{ routes : "generates"
    routes ||--o{ route_pois : "contains"
    pois ||--o{ route_pois : "is part of"
```

#### 3.3.2. Detailed Schema Design

**`users`**: Stores user account information.

|              |               |                                            |                             |
| ------------ | ------------- | ------------------------------------------ | --------------------------- |
| **Column**   | **Type**      | **Constraints**                            | **Description**             |
| `id`         | `UUID`        | `PRIMARY KEY`, `DEFAULT gen_random_uuid()` | Unique user identifier.     |
| `created_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()`                | Timestamp of user creation. |
| `updated_at` | `TIMESTAMPTZ` | `NOT NULL`, `DEFAULT NOW()`                | Timestamp of last update.   |

**`interest_themes`**: A lookup table for available interest themes.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`id`|`UUID`|`PRIMARY KEY`, `DEFAULT gen_random_uuid()`|Unique theme identifier.|
|`name`|`TEXT`|`NOT NULL`, `UNIQUE`|The display name of the theme (e.g., "Street Art").|
|`description`|`TEXT`||A short description of the theme.|

**`user_interest_themes`**: A join table linking users to their selected themes.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`user_id`|`UUID`|`PRIMARY KEY`, `FOREIGN KEY (users.id)`|Foreign key to the `users` table.|
|`theme_id`|`UUID`|`PRIMARY KEY`, `FOREIGN KEY (interest_themes.id)`|Foreign key to the `interest_themes` table.|

**`pois`**: Stores all Points of Interest.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`id`|`BIGINT`|`PRIMARY KEY`|OSM Node/Way/Relation ID.|
|`location`|`GEOGRAPHY(Point, 4326)`|`NOT NULL`|The geographic coordinate of the POI. Indexed with GIST.|
|`osm_tags`|`JSONB`||Flexible key-value store for all OSM tags. Indexed with GIN.|
|`narrative`|`TEXT`||Engaging description, potentially LLM-generated.|
|`popularity_score`|`REAL`|`DEFAULT 0.5`|A score from 0.0 (hidden gem) to 1.0 (hotspot).|

**`poi_interest_themes`**: A join table linking POIs to relevant themes.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`poi_id`|`BIGINT`|`PRIMARY KEY`, `FOREIGN KEY (pois.id)`|Foreign key to the `pois` table.|
|`theme_id`|`UUID`|`PRIMARY KEY`, `FOREIGN KEY (interest_themes.id)`|Foreign key to the `interest_themes` table.|

**`routes`**: Stores generated walks.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`id`|`UUID`|`PRIMARY KEY`, `DEFAULT gen_random_uuid()`|Unique identifier for the generated route.|
|`user_id`|`UUID`|`FOREIGN KEY (users.id)`|The user who generated the route (can be NULL for anonymous users).|
|`path`|`GEOMETRY(LineString, 4326)`||The full geometric path of the walk. Indexed with GIST for spatial queries.|
|`generation_params`|`JSONB`||A snapshot of the parameters used to generate this route (duration, themes, accessibility, etc.).|
|`created_at`|`TIMESTAMPTZ`|`NOT NULL`, `DEFAULT NOW()`|Timestamp of route generation.|

**`route_pois`**: A join table to store the ordered sequence of POIs for a specific route.

|   |   |   |   |
|---|---|---|---|
|**Column**|**Type**|**Constraints**|**Description**|
|`route_id`|`UUID`|`PRIMARY KEY`, `FOREIGN KEY (routes.id)`|Foreign key to the `routes` table.|
|`poi_id`|`BIGINT`|`PRIMARY KEY`, `FOREIGN KEY (pois.id)`|Foreign key to the `pois` table.|
|`order`|`INTEGER`|`NOT NULL`|The sequential position of this POI in the walk (e.g., 1, 2, 3...).|

#### 3.3.3. MVP Data Strategy: Hybrid Ingestion and Caching

To balance the need for performance with the challenges of data volume, freshness, and relevance, we will adopt a three-layered hybrid strategy for the MVP.

```
flowchart TD
    A[User requests route via POST /routes] --> B{Check Redis for cached route};
    B -- Yes --> C[Return cached route ID];
    B -- No --> D[Enqueue GenerateRouteJob];
    D --> E{Worker picks up GenerateRouteJob};
    E --> F[Query local PostGIS for POIs];
    F --> G{Is local data sufficient?};
    G -- Yes --> H[Generate route from local data];
    G -- No --> I[Generate best-effort route from local data];
    I --> J[Enqueue low-priority EnrichmentJob];
    H --> K[Write route to DB];
    J --> K;
    K --> L[Write route to Redis cache];
    L --> M[Push 'complete' notification via WebSocket];

    subgraph "Low-Priority Background Task"
    Z[EnrichmentJob] --> Y[Query Overpass API for missing POIs];
    Y --> X[Upsert new POIs into PostGIS];
    end
```

- **Layer 1: Foundational Pre-Seeding (The "Hot Cache").** The core `wildside-engine` requires a fast, local data source for its intensive queries. For the MVP, we will perform a **one-time, geographically scoped data ingestion**.
    
    - **Scope:** A defined polygon covering our initial launch area (e.g., the City of Edinburgh).
        
    - **Process:** A script will download a regional OSM extract (e.g., from Geofabrik), filter for a comprehensive baseline of common POI tags (`amenity`, `historic`, `tourism`, `leisure`, `natural`), and ingest this data into our PostGIS `pois` table.
        
    - **Purpose:** This guarantees that the vast majority of route requests have a rich, local dataset to draw from, ensuring the "time to first walk" is consistently fast.
        
- **Layer 2: On-Demand Enrichment (The "Warm Cache").** This layer addresses the "cold start" problem for niche interests and ensures the dataset evolves based on user demand, not just our assumptions.
    
    - **Trigger:** When the `GenerateRouteJob` queries the local database and finds a sparse set of results for a user's chosen theme in a given area (e.g., fewer than a threshold of `N` POIs), it triggers this process.
        
    - **Process:** The worker proceeds to generate the best route it can with the limited local data. Crucially, it also enqueues a _separate, low-priority `EnrichmentJob`_. This new job will perform a targeted query against an external source (like the Overpass API) for the missing POI types in that geographic bounding box. The results are then inserted or updated (`UPSERT`) into our `pois` table.
        
    - **Purpose:** This enriches our local dataset precisely where it was found lacking. The first user interested in "brutalist architecture" gets a reasonable walk immediately, but in doing so, they trigger a process that ensures the next user gets a fantastic one. This solves the problems of data sparseness and making incorrect assumptions about user taste.
        
- **Layer 3: Route Output Caching.** This concerns the _results_ of the computation, not the source data.
    
    - **Process:** When a route is successfully generated, its full definition is cached in Redis. The cache key will be a hash of the precise request parameters (location, duration, themes, etc.). Saved routes will have their cache TTL removed, effectively pinning them.
        
    - **Purpose:** This prevents re-computation for identical requests, providing an instantaneous response for popular or repeated queries.
        

#### 3.3.4. Implementation Tasks

- [ ] **Initial Data Seeding:** Create a standalone data ingestion script (e.g., using Python with `osmium`) that performs the one-time pre-seeding of the database for a defined geographic area (Edinburgh).
    
- [ ] **Implement On-Demand Enrichment Logic:** In the `GenerateRouteJob` handler, add logic to detect when the local POI query returns a sparse result set. If triggered, this logic should enqueue a follow-up `EnrichmentJob` with the relevant parameters (bounding box, interest themes).
    
- [ ] **Implement Schema Migrations:** Create the database schema using Diesel migration files.
    

### 3.4. Background Task Workers

Asynchronous and long-running tasks are executed by a separate pool of worker processes to avoid blocking the main API server.

- **Technology:** Apalis (with a Redis or Postgres backend).
    
- **Current Status:** This component is purely at the design stage. No implementation exists.
    
- **Key Responsibilities:**
    
    - Execute computationally intensive jobs (`GenerateRouteJob`).
        
    - Perform data enrichment tasks (`EnrichmentJob`).
        
    - Perform periodic maintenance tasks (e.g., refreshing data from external sources).
        
- **Implementation Tasks:**
    
    - [ ] **Integration:**
        
        - Add `apalis` to `backend/Cargo.toml`.
            
        - Configure Apalis to use Redis as the job queue broker, with separate queues for high-priority (e.g., `route_generation`) and low-priority (`enrichment`) tasks.
            
    - [ ] **Worker Binary:** Modify `main.rs` to launch in "worker" mode based on a CLI flag or environment variable (`WILDSIDE_MODE=worker`). In this mode, it should start the Apalis worker pool.
        
    - [ ] **Job Definitions:**
        
        - Define and implement the `GenerateRouteJob` as previously described. It should now include the logic to trigger the `EnrichmentJob`.
            
        - Define and implement the `EnrichmentJob`. This job's handler will construct and execute a query against the Overpass API and use Diesel to `UPSERT` the results into the `pois` table.
            
    - [ ] **Deployment:** Create a second Kubernetes `Deployment` for the workers.
        

### 3.5. Caching Layer

An in-memory cache is used to improve performance and reduce database load.

- **Technology:** Redis
    
- **Current Status:** This component is purely at the design stage.
    
- **Key Responsibilities:**
    
    - Cache the results of expensive, deterministic operations, such as route generation for common parameters.
        
    - Cache frequently accessed, slow-changing data from the database (e.g., popular POIs).
        
- **Implementation Tasks:**
    
    - [ ] **Integration:** Add the `redis` crate and configure a Redis connection pool available to the Actix application state.
        
    - [ ] **Route Caching:**
        
        - Before enqueuing a `GenerateRouteJob`, the API handler must first check Redis for a cached result. The cache key should be a hash of the route request parameters.
            
        - On successful route generation, the background worker must write the result to the cache with a reasonable TTL (e.g., 24 hours).
            

### 3.6. Observability

The system must be fully instrumented to provide insight into its performance, reliability, and user behaviour.

- **Technology:** Prometheus, Grafana, Loki, PostHog, `tracing` crate.
    
- **Current Status:** `tracing` is integrated for basic logging. The Kubernetes manifests are configured to support the Prometheus Operator.
    
- **Key Responsibilities:**
    
    - **Metrics (Prometheus):** Expose key operational metrics for monitoring and alerting.
        
    - **Logging (Loki):** Output structured, correlated logs for debugging.
        
    - **Analytics (PostHog):** Send events to track user engagement and product funnels.
        
- **Implementation Tasks:**
    
    - [ ] **Metrics:** In addition to the `actix-web-prom` metrics, implement the following custom application metrics:
        
        - A histogram (`route_generation_duration_seconds`) to track the execution time of the `GenerateRouteJob`.
            
        - A counter (`jobs_total{type,status}`) to track the number of background jobs processed (e.g., type=`GenerateRoute`, status=`success|failure`).
            
        - A gauge (`websocket_connections_active`) for the number of connected WebSocket clients.
            
        - A counter (`enrichment_jobs_total{status}`) to track the success/failure of data enrichment jobs.
            
        - A gauge (`pois_total`) for the total number of POIs in the local database, to observe growth over time.
            
    - [ ] **Logging:** Ensure all logs are emitted as structured JSON and include the `trace_id` propagated from the initial API request, even into the background jobs.
        
    - [ ] **Analytics:**
        
        - Integrate the PostHog Rust client.
            
        - Send a `RouteComputed` event from the background worker upon successful route generation, including properties like `route_duration_minutes` and `poi_count`.
            
        - Send `UserSignup` and `UserLogin` events from the relevant API endpoints.
            

## 4. API and WebSocket Specification

This section defines the API contracts for client-server communication.

### 4.1. REST API (v1)

All REST endpoints are prefixed with `/api/v1`.

#### User & Session Management

|   |   |   |   |
|---|---|---|---|
|**Method**|**Path**|**Description**|**Authentication**|
|`POST`|`/users`|Creates a new anonymous user session.|None|
|`GET`|`/users/me`|Retrieves the current user's profile and preferences.|Session Cookie|
|`PUT`|`/users/me/interests`|Updates the current user's selected interest themes.|Session Cookie|

#### Content

|   |   |   |   |
|---|---|---|---|
|**Method**|**Path**|**Description**|**Authentication**|
|`GET`|`/interest-themes`|Retrieves the list of all available interest themes.|None|

#### Routes

|   |   |   |   |
|---|---|---|---|
|**Method**|**Path**|**Description**|**Authentication**|
|`POST`|`/routes`|Submits a request to generate a new walking route.|Session Cookie|
|`GET`|`/routes/{route_id}`|Retrieves a previously generated route by its ID.|Session Cookie|
|`GET`|`/users/me/routes`|Retrieves a list of routes generated by the current user.|Session Cookie|

**`POST /routes` Request Body:**

```
{
  "start_location": {
    "type": "Point",
    "coordinates": [-3.1883, 55.9533]
  },
  "duration_minutes": 60,
  "interest_theme_ids": [
    "f47ac10b-58cc-4372-a567-0e02b2c3d479"
  ],
  "popularity_bias": 0.7,
  "accessibility": {
    "avoid_stairs": true,
    "prefer_well_lit": false
  }
}
```

On success, this endpoint returns a `202 Accepted` with a body containing the `request_id` and the `route_id` for the pending resource.

### 4.2. WebSocket API

The WebSocket is available at `/ws`. After connection, the client is implicitly subscribed to notifications for their user ID, which is identified via the session cookie provided during the handshake.

#### Server-to-Client Messages

**`route_generation_status`**

Pushed to the client to provide real-time updates on a route generation job.

- **`type`**: `"route_generation_status"`
    
- **Payload**:
    
    - `request_id` (string): Correlates with the ID returned from `POST /routes`.
        
    - `status` (string): One of `pending`, `in_progress`, `complete`, `failed`.
        
    - `route_id` (string, optional): The ID of the final route, present when status is `complete`.
        
    - `error` (string, optional): An error message, present when status is `failed`.
        

#### Client-to-Server Messages

**`update_location`** (For future "In-Walk Navigation" features)

Sent periodically by the client to update the server with their current location during an active walk.

- **`type`**: `"update_location"`
    
- **Payload**:
    
    - `route_id` (string): The ID of the route being navigated.
        
    - `location` (GeoJSON Point): The user's current coordinates.
        

## 5. Tile Serving Architecture

To deliver a rich, interactive, and highly performant map experience, the application will not rely on external third-party map providers for our dynamic data. Instead, we will serve our own vector tiles directly from the application's PostGIS database. This gives us complete control over map styling, data representation, and performance.

- **Technology:** [Martin](https://martin.maplibre.org/ "null"), a high-performance vector tile server written in Rust.
    
- **Strategy:** Martin will be deployed as a separate, stateless service within our Kubernetes cluster. It will connect directly to the primary PostGIS database (ideally with a read-only user) and expose tile endpoints that can be consumed by the frontend PWA (using a library like MapLibre GL JS).
    

### 5.1. Architectural Integration

The tile server is a distinct service, separate from the main Wildside backend monolith. This separation of concerns is crucial: the backend handles business logic, authentication, and orchestration, while Martin's sole responsibility is the efficient generation and serving of map tiles.

```
flowchart TD
    subgraph "Frontend PWA"
        A[MapLibre GL JS Client]
    end

    subgraph "Kubernetes Cluster"
        B[Ingress<br>/tiles/{source}/{z}/{x}/{y}] --> C[Martin Service];
        C -- SQL / Function Call --> D[(PostGIS Database)];
        D -- Tables, Views, Functions --> C;
        C -- Vector Tile (PBF) --> B;
        
        E[Wildside Backend API] --> D;
    end
    
    A -- Requests Tiles --> B;
```

### 5.2. MVP Tile Sources

For the MVP, Martin will be configured to serve the following dynamic tile sources. These sources will be defined in Martin's configuration file (`config.yaml`).

#### 5.2.1. Points of Interest (`pois`)

This layer will expose our curated and enriched POIs, allowing the frontend to display them dynamically based on zoom level and user context.

- **Source Type:** Table
    
- **Table Name:** `public.pois`
    
- **Endpoint:** `/tiles/pois/{z}/{x}/{y}.pbf`
    
- **Implementation:** Martin can serve this directly. We will select which columns from the `pois` table are included as properties in the tile features (e.g., `id`, `popularity_score`) to keep the tile size minimal. Full POI details will be fetched from the main REST API when a user interacts with a point.
    

#### 5.2.2. Generated Routes (`routes`)

This layer will display a specific, user-generated route on the map. As routes are user-specific and generated on demand, we cannot simply serve the entire `routes` table. A PostGIS function is the ideal solution.

- **Source Type:** Function
    
- **Endpoint:** `/tiles/routes/{route_id}/{z}/{x}/{y}.pbf`
    
- **Implementation:**
    
    1. Create a PostGIS function, e.g., `get_route_tile(route_id UUID, z integer, x integer, y integer)`.
        
    2. This function will take the `route_id` from the URL path as a parameter.
        
    3. It will query the `routes` table for the matching `path` geometry.
        
    4. It will use PostGIS functions like `ST_AsMVTGeom` and `ST_AsMVT` to generate the vector tile for the requested `z/x/y` coordinate, containing only the relevant segment of that specific route's linestring.
        
    5. Martin will be configured to call this function, passing in the parameters from the request URL.
        

#### 5.2.3. Base Network (Optional Post-MVP)

For greater control over styling and to reduce reliance on external providers entirely, we could serve our own base layer of roads, paths, and land use polygons. This would involve ingesting more comprehensive OSM data (e.g., from `planet_osm_line`, `planet_osm_polygon`) and creating table or function sources for them. This is not required for the MVP but is a logical next step.

### 5.3. Implementation Tasks

- [ ] **Create Martin Docker Image:** Although a pre-built image exists, we may want to build our own to bundle our specific `config.yaml`.
    
- [ ] **Kubernetes Deployment:** Create a new `Deployment` and `Service` manifest for the Martin tile server in our Kubernetes configuration.
    
- [ ] **Ingress Configuration:** Add a new rule to the Traefik `IngressRoute` to direct traffic from a subdomain (e.g., `tiles.wildside.app`) or a path prefix (e.g., `/tiles`) to the Martin service.
    
- [ ] **Martin Configuration:** Create a `config.yaml` file for Martin. This file will define the database connection string and specify the tile sources for `pois` and `routes` as detailed above. This config should be mounted into the Martin pod via a `ConfigMap`.
    
- [ ] **Create PostGIS Function:** Implement and test the `get_route_tile` SQL function in a new Diesel migration file. Ensure it is performant and returns correctly formatted vector tile data.
    
- [ ] **Observability:** Configure the Prometheus Operator to scrape the `/metrics` endpoint that Martin exposes by default, and create a new Grafana dashboard to monitor tile serving performance (e.g., request latency, cache hit rates, error rates).
