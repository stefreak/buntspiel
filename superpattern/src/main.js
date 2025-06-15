//// BUNTSPIEL PIXELBLAZE SUPERPATTERN

var ORIG_PIXELCOUNT = pixelCount
var ORIG_MAPPIXELS = mapPixels
// TODO: overrides
// pixelCount
// mapPixels

// TODO: reset funcitons with side effects that can affect other patterns
// transform, translate, scale, rotate, translate3D, scale3D, rotateX, rotateY, rotateZ => call resetTransform
// setPerlinWrap => reset to default
// prngSeed => remember seed somehow for next invocation?

// TODO: side effect combinators
// hsv
// hsv24
// rgb
// setPalette
// paint

// TODO: what do we do with I/O functions?
// readAdc, analogRead, pinMode, digitalWrite, digitalRead, touchRead

// Buntspiel cube consists of 4 sides, each with 4x4 = 16 pixels
var BUNTSPIEL_N_SIDES = 4
var BUNTSPIEL_SIDE_PIXELS = 16
// first 64 pixels are reserved for buntspiel
var BUNTSPIEL_PIXELS = BUNTSPIEL_N_SIDES * BUNTSPIEL_SIDE_PIXELS

// TODO: implement combinator functions
var COMBINATOR_ADD = 0
var COMBINATOR_SUB = 1
var COMBINATOR_AVG = 2
var COMBINATOR_MASK = 3

// patterns are represented as arrays with the render, render2d, render3d and beforeRender functions
// These constants can be used to index into the correct pattern callback
var PATTERN_RENDER = 0
var PATTERN_RENDER2D = 1
var PATTERN_RENDER3D = 2
var PATTERN_BEFORE_RENDER = 3
var PATTERN_STATE = 4

/**
 * @type (() => [
 *  ((index: number, state: State) => void) || 0,
 *  ((index: number, x: number, y: number) => void) || 0,
 *  ((index: number, x: number, y: number, z: number) => void) || 0,
 *  ((delta: number) => void) || 0,
 *  [], // state
 * ])[]
 */
var PATTERN_CONSTRUCTORS = [/** AUTOGENERATE_PATTERNS **/]
var NOOP_PATTERN_CONSTRUCTOR = [() => [0, 0, 0, 0, []]]

// The cube consists 4 sides, each with 4x4 pixels
// We divide each side into 4 faces (each 2x2 pixels) and display a preview
var faces = array(4 * 4)

// Up to 10 patterns can be combined in Glühtürmchen. Default is no pattern.
var turm = array(10)
turm[0] = [COMBINATOR_ADD, NOOP_PATTERN]

// We start with the first 16 patterns spread onto 16 faces
arrayMapTo(patterns, faces, (pattern) => {
    overrideGlobals(4)
    return pattern()
})

function overrideGlobals(overridePixelCount) {
    // TODO: implement override for all the other globals
    pixelCount = overridePixelCount
}

// run a render function from a pattern
function call_pattern(pattern, renderfn_index, index, x, y, z, delta) {
    var state = pattern[PATTERN_STATE]

    while (renderfn_index >= 0) {
        if (pattern[renderfn_index] != 0) {
            if (renderfn_index == PATTERN_BEFORE_RENDER) {
                pattern[PATTERN_STATE] = pattern[renderfn_index](delta, state)
            } else if (renderfn_index == PATTERN_RENDER3D) {
                pattern[PATTERN_STATE] = pattern[renderfn_index](index, x, y, z, state)
            } else if (renderfn_index == PATTERN_RENDER2D) {
                pattern[PATTERN_STATE] = pattern[renderfn_index](index, x, y, state)
            } else if (renderfn_index == PATTERN_RENDER) {
                pattern[PATTERN_STATE] = pattern[renderfn_index](index, state)
            }
            return
        }

        // Do not fall back to other render functions when calling beforeRender
        if (renderfn_index == PATTERN_BEFORE_RENDER) {
            return
        }

        // try the next function if this one didn't exist; render3d => render2d => render
        renderfn_index -= 1;
    }
}

function render_buntspiel(renderfn_index, index, x, y, z, delta) {
    var side = floor(index / BUNTSPIEL_SIDE_PIXELS)
    index -= BUNTSPIEL_SIDE_PIXELS * side // Now it's index in the current side

    var row = floor(index / 4)
    var col = index % 4

    // finally, calculate the face index
    var face = (floor(row / 2) * 2 + floor(col / 2))
    var face_index = col % 2  + row % 2  * 2; 

    // test cases:
    // { index: 0, row: 0, col: 0, face: 0, face_index: 0 },
    // { index: 1, row: 0, col: 1, face: 0, face_index: 1 },
    // { index: 2, row: 0, col: 2, face: 1, face_index: 0 },
    // { index: 3, row: 0, col: 3, face: 1, face_index: 1 },
    // { index: 4, row: 1, col: 0, face: 0, face_index: 2 },
    // { index: 5, row: 1, col: 1, face: 0, face_index: 3 },
    // { index: 6, row: 1, col: 2, face: 1, face_index: 2 },
    // { index: 7, row: 1, col: 3, face: 1, face_index: 3 },
    // { index: 8, row: 2, col: 0, face: 2, face_index: 0 },
    // { index: 9, row: 2, col: 1, face: 2, face_index: 1 },
    // { index: 10, row: 2, col: 2, face: 3, face_index: 0 },
    // { index: 11, row: 2, col: 3, face: 3, face_index: 1 },
    // { index: 12, row: 3, col: 0, face: 2, face_index: 2 },
    // { index: 13, row: 3, col: 1, face: 2, face_index: 3 },
    // { index: 14, row: 3, col: 2, face: 3, face_index: 2 },
    // { index: 15, row: 3, col: 3, face: 3, face_index: 3 }

    overrideGlobals(4)
    call_pattern(faces[side * 4 + face], renderfn_index, face_index, row, col, 0, delta)
}

function render_turmchen(renderfn_index, index, x, y, z, delta) {
    index -= BUNTSPIEL_PIXELS
    // TODO
}

// standard callbacks. pixelblaze renderer will call these
function render(index) {
    if (index < BUNTSPIEL_PIXELS) {
        render_buntspiel(PATTERN_RENDER, index)
    } else {
        render_turmchen(PATTERN_RENDER, index)
    }
}
function render2d(index, x, y) {
    if (index < BUNTSPIEL_PIXELS) {
        render_buntspiel(PATTERN_RENDER2D, index, x, y)
    } else {
        render_turmchen(PATTERN_RENDER2D, index, x, y)
    }
}
function render3d() {
    if (index < BUNTSPIEL_PIXELS) {
        render_buntspiel(PATTERN_RENDER3D, index, x, y, z)
    } else {
        render_turmchen(PATTERN_RENDER3D, index, x, y, z)
    }
}
function beforeRender(delta) {
    if (index < BUNTSPIEL_PIXELS) {
        render_buntspiel(PATTERN_BEFORE_RENDER, 0, 0, 0, 0, delta)
    } else {
        render_turmchen(PATTERN_BEFORE_RENDER, 0, 0, 0, 0, delta)
    }
}
