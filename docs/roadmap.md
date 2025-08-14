# Wildside roadmap

* [ ] **Core Product Experience & Feature Set (MVP)**

  * [ ] **Onboarding & Personalization**

    * [ ] Implement a visual grid of "Interest Themes" for initial user
            selection.

    * [ ] Connect theme selections to the Point of Interest (POI) scoring
            algorithm's weighting parameters.

  * [ ] **Interactive Map & Discovery**

    * [ ] Develop a clean, high-performance map interface as the
            application's home screen.

    * [ ] Make POIs tappable to reveal a concise summary from enriched
            Wikidata data.

  * [ ] **Route Generation Controls**

    * [ ] Create a "Generate Walk" button with a minimalist modal.

    * [ ] Implement a duration slider (15 minutes to 3 hours).

    * [ ] Add interest theme toggles to tailor each walk.

    * [ ] Allow users to set a start and end point for the route,
            defaulting to their current location.

  * [ ] **In-Walk Navigation Experience**

    * [ ] Optimize the navigation interface for a clear,
            pedestrian-focused experience.

    * [ ] Implement offline capability for map tiles and navigation logic.

    * [ ] Proactively highlight upcoming POIs on the route.

    * [ ] Allow tapping on a POI to reveal a text-based narrative snippet.

  * [ ] **Post-Walk Summary**

    * [ ] Create a summary screen displaying the path, distance, and
            duration.

    * [ ] Include a visual gallery of discovered POIs.

* [ ] **Recommendation Engine & Data Foundation**

  * [ ] **Data Sourcing and Processing**

    * [ ] Set up an ETL pipeline to ingest data from OpenStreetMap and
            Wikidata.

    * [ ] Use `osm2pgsql` to transform and load OSM data into a PostGIS
            database.

    * [ ] Develop a script to link and enrich OSM data with Wikidata
            properties.

  * [ ] **POI Scoring & Personalization**

    * [ ] Implement a dynamic scoring model for POIs based on global
            popularity and user relevance.

    * [ ] Calculate global popularity based on Wikidata properties (e.g.,
            Wikipedia article presence, UNESCO designation).

    * [ ] Map user "Interest Themes" to specific Wikidata properties to
            calculate user relevance.

  * [ ] **Core Routing Engine**

    * [ ] Formulate the route generation as an Orienteering Problem.

    * [ ] Use heuristics (e.g., Greedy Insertion, 2-Opt) to find
            high-quality, near-optimal solutions.

    * [ ] Integrate Google OR-Tools as the solver for the routing problem.

* [ ] **System Architecture & Technology Stack (MVP)**

  * [ ] **Backend Services**

    * [ ] Develop a monolithic backend using Rust with the Actix Web
            framework.

    * [ ] Set up a PostgreSQL database with the PostGIS extension.

    * [ ] Utilize Diesel as the ORM for safe, compile-time query
            validation.

  * [ ] **Frontend Application (PWA)**

    * [ ] Build the frontend as a Progressive Web App using TypeScript
            and React with Vite.

    * [ ] Use DaisyUI with Tailwind CSS for UI components and styling.

    * [ ] Manage server state with TanStack Query.

    * [ ] Render vector maps using MapLibre GL JS.

  * [ ] **AI/LLM Integration**

    * [ ] Implement on-device intent recognition using a small, local LLM
            (e.g., Mistral 7B).

    * [ ] Use a cloud-based LLM API (e.g., Claude 3.5 Sonnet, GPT-4o
            mini) for narrative generation.

    * [ ] Implement aggressive caching and prompt optimization to manage
            LLM costs.
