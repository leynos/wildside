# Wildside PWA Sitemap

## Route Structure

| Path                    | Component                   | Description                                       |
| ----------------------- | --------------------------- | ------------------------------------------------- |
| `/`                     | Redirect                    | Redirects to `/welcome`                           |
| `/welcome`              | `WelcomeScreen`             | Landing page with branding and value propositions |
| `/discover`             | `DiscoverScreen`            | Interest selection onboarding                     |
| `/explore`              | `ExploreScreen`             | Route catalogue and discovery                     |
| `/customize`            | `CustomizeScreen`           | Walk customiser with preferences                  |
| `/wizard`               | Redirect                    | Redirects to `/wizard/step-1`                     |
| `/wizard/step-1`        | `WizardStepOne`             | Route preferences wizard (step 1)                 |
| `/wizard/step-2`        | `WizardStepTwo`             | Route preferences wizard (step 2)                 |
| `/wizard/step-3`        | `WizardStepThree`           | Route preferences wizard (step 3)                 |
| `/map`                  | Layout route                | Wraps child routes with MapStateProvider          |
| `/map/quick`            | `QuickWalkScreen`           | Quick walk generation and preview                 |
| `/map/itinerary`        | `ItineraryScreen`           | Detailed walk itinerary view                      |
| `/saved`                | `SavedScreen`               | Saved walk details                                |
| `/walk-complete`        | `WalkCompleteScreen`        | Walk completion summary                           |
| `/offline`              | `OfflineScreen`             | Offline map manager                               |
| `/safety-accessibility` | `SafetyAccessibilityScreen` | Safety preferences                                |

## Navigation Groups

### Primary Navigation (Bottom Nav)

The map-related screens share a common bottom navigation:

- **Map** (`/map/quick`) - Quick walk generation
- **Discover** (`/explore`) - Route catalogue
- **Routes** (`/customize`) - Walk customiser
- **Profile** (`/offline`) - Offline settings

### Nested Routes

The `/map` layout route provides shared state via `MapStateProvider`:

```text
/map
├── /quick      (QuickWalkScreen)
└── /itinerary  (ItineraryScreen)
```

## State Transition Diagram

```mermaid
stateDiagram-v2
    [*] --> Welcome: App launch

    Welcome --> Discover: Get started
    Discover --> Explore: Start exploring

    Explore --> WizardStep1: Start wizard
    Explore --> Customize: Quick customise
    Explore --> SafetyAccessibility: Settings

    Customize --> MapQuick: Generate route

    WizardStep1 --> WizardStep2: Next
    WizardStep1 --> Explore: Back

    WizardStep2 --> WizardStep3: Next
    WizardStep2 --> WizardStep1: Back

    WizardStep3 --> MapQuick: Generate route
    WizardStep3 --> WizardStep2: Back
    WizardStep3 --> WizardStep1: Reset

    MapQuick --> Itinerary: View details
    MapQuick --> Saved: Save route
    MapQuick --> WizardStep1: Customise
    MapQuick --> Offline: Download

    Itinerary --> MapQuick: Back

    Saved --> MapQuick: Back

    WalkComplete --> Saved: Save
    WalkComplete --> WizardStep1: Remix

    Offline --> MapQuick: Back

    SafetyAccessibility --> Explore: Done

    note right of Welcome
        Entry point (redirects from /)
    \nend note

    note right of MapQuick
        Primary map view
        Shared MapStateProvider
    \nend note

    note right of WizardStep3
        Route preview and
        final customisation
    \nend note
```

## Feature Modules

| Feature       | Route(s)                | Key Components                                      |
| ------------- | ----------------------- | --------------------------------------------------- |
| Welcome       | `/welcome`              | `WelcomeScreen`                                     |
| Discover      | `/discover`             | `DiscoverScreen`                                    |
| Explore       | `/explore`              | `ExploreScreen`                                     |
| Customize     | `/customize`            | `CustomizeScreen`                                   |
| Wizard        | `/wizard/*`             | `WizardStepOne`, `WizardStepTwo`, `WizardStepThree` |
| Map           | `/map/*`                | `QuickWalkScreen`, `ItineraryScreen`                |
| Saved         | `/saved`                | `SavedScreen`                                       |
| Walk Complete | `/walk-complete`        | `WalkCompleteScreen`                                |
| Offline       | `/offline`              | `OfflineScreen`                                     |
| Safety        | `/safety-accessibility` | `SafetyAccessibilityScreen`                         |

## User Flows

### New User Onboarding

1. App launches → `/welcome` (landing page)
2. User taps "Get started" → `/discover` (interest selection)
3. User taps "Start exploring" → `/explore` (catalogue)
4. User selects preferences → `/wizard/step-1`
5. Complete wizard → `/map/quick` (generated route)

### Quick Route Generation

1. From `/explore` → `/customize`
2. Adjust sliders and options → `/map/quick`
3. View details → `/map/itinerary`

### Route Completion

1. Finish walk → `/walk-complete`
2. Save route → `/saved`
3. Remix route → `/wizard/step-1`
