# Rendering & UI

## Technology

- **SDL2 0.38**: Window management, 2D canvas rendering, input events
- **fontdue 0.9**: CPU-based font rasterization (no SDL2_ttf dependency)
- **image 0.25**: PNG decoding for embedded sprites

All assets (sprites, background, font) are embedded at compile time via `include_bytes!()`.

## Window

- Resolution: 1280×720
- Frame rate: ~60 FPS (16ms sleep per frame)
- Blend mode enabled for alpha transparency

## Text Rendering

Text is rendered using fontdue's CPU rasterizer with Source Code Pro SemiBold font. Each text string is rasterized into an SDL2 texture in ABGR8888 format. Text color and alpha are configurable per render call.

Scenes pre-render text textures where possible (menu items, letter bar) and re-render only when content changes.

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
│  L2/R2: Letter    X: Download    B: Back   │  Legend
└────────────────────────────────────────────┘
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
