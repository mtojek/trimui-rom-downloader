# Rendering & UI

## Technology

- **SDL2 0.38**: Window management, 2D canvas rendering, input events
- **fontdue 0.9**: CPU-based font rasterization (no SDL2_ttf dependency)
- **image 0.25**: PNG decoding for embedded sprites

All assets (sprites, background, font) are embedded at compile time via `include_bytes!()`.

## Window

- Resolution: 1280×720
- Frame rate: ~60 FPS (event-driven via `wait_event_timeout(16)`)
- Blend mode enabled for alpha transparency

## Text Rendering

Text is rendered using fontdue's CPU rasterizer with Source Code Pro SemiBold font. Each text string is rasterized into an SDL2 texture in ABGR8888 format. Text color and alpha are configurable per render call.

### Texture Caching

Creating a `TextRenderer` parses the TTF font from scratch and `render_text()` rasterizes every glyph — this is expensive. Scenes **must** pre-render text textures and cache them, only re-rendering when content changes:

- **Browser**: letter bar, game list entries, and legend are cached. Legend is re-rendered only when the selected entry type changes (downloadable vs installed).
- **MyGames**: all row textures, legend, "No games yet" text, and delete confirmation dialog are cached. Legend updates only when the selected row type/state changes.
- **Static textures** (titles, dialog buttons) are created once and reused for the lifetime of the scene.

## Scene Rendering

Each scene renders directly to the SDL2 canvas. Common patterns:

- **Dark background boxes**: semi-transparent black rectangles behind content
- **Color coding**: green (installed), blue (downloading), purple (unpacking), orange (paused), red (failed), gray (queued), yellow (selected)
- **Legend bar**: bottom of screen, contextual per scene and selected item state
- **Progress bars**: horizontal bars with background track and colored fill

## Intro Animation

Timed sequence using elapsed milliseconds:
1. Crab sprites fade in with oscillating rotation
2. Background alpha ramps up
3. Game cart slides down with ease-out curve `1-(1-t)²`
4. Everything slides up off screen
5. Transition to Menu after 4200ms

## Game Browser Layout

```
┌────────────────────────────────────────────┐
│  # A B C D E F G H I J K L M              │  Letter bar row 1
│  N O P Q R S T U V W X Y Z                │  Letter bar row 2
├────────────────────────────────────────────┤
│  Game Name.chd                    650 MB   │  15 visible rows
│  Another Game.zip                 420 MB   │  with scroll
│  ...                                       │
├────────────────────────────────────────────┤
│  L/R: Letter    X: Download    B: Back     │  Legend
└────────────────────────────────────────────┘
```

### Controls

- **D-Pad Up/Down**: move cursor one item
- **D-Pad Left/Right**: page up/down (jump by one screen, cursor lands at top)
- **L/R shoulders, L2/R2 triggers**: switch letter
- **X (action)**: start download
- **B**: go back
```

## My Games Layout

```
┌────────────────────────────────────────────┐
│                 My Games                    │  Title
├────────────────────────────────────────────┤
│  Downloading Game.chd    650 MB  45% 2MB/s │  Active downloads
│  Queued Game.zip         420 MB  Queued     │
│  ──────────────────────────────────────     │  Separator
│  Installed Game          PlayStation        │  Installed games
│  ...                                       │
├────────────────────────────────────────────┤
│  B: Back    X: Pause    Y: Delete          │  Contextual legend
└────────────────────────────────────────────┘
```

Download rows include a progress bar below the text when active, paused, or unpacking.

### Controls

- **D-Pad Up/Down**: move cursor one item
- **D-Pad Left/Right**: page up/down (jump by one screen, cursor lands at top)
- **X (action)**: pause/resume/retry download
- **Y (refresh)**: delete (with confirmation dialog)
- **B**: go back

## Performance

### Event-Driven Main Loop

The main loop uses `SDL_WaitEventTimeout(16)` instead of `poll + sleep`. When idle, SDL yields the CPU to the OS, resulting in near-zero CPU usage when no input or downloads are active.

### Throttled Refresh

Scenes that display download status (Browser, MyGames) use throttled refresh:

- **No active downloads**: refresh is skipped entirely (zero mutex locks, zero iterations)
- **Active downloads**: refresh runs at most every 500ms
- **Download events** (completed/failed): trigger an immediate refresh regardless of throttle

This avoids the cost of rebuilding textures 60 times per second when nothing has changed.
