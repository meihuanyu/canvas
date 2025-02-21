const { loadBinding } = require('@node-rs/helper')

/**
 * __dirname means load native addon from current dir
 * 'skia' means native addon name is `skia`
 * the first arguments was decided by `napi.name` field in `package.json`
 * the second arguments was decided by `name` field in `package.json`
 * loadBinding helper will load `skia.[PLATFORM].node` from `__dirname` first
 * If failed to load addon, it will fallback to load from `@napi-rs/skia-[PLATFORM]`
 */
const {
  CanvasRenderingContext2D,
  CanvasElement,
  SVGCanvas,
  Path2D,
  ImageData,
  Image,
  CanvasPattern,
  GlobalFonts,
  convertSVGTextToPath: _convertSVGTextToPath,
} = loadBinding(__dirname, 'skia', '@napi-rs/canvas')

const Geometry = require('./geometry')

const StrokeJoin = {
  Miter: 0,
  Round: 1,
  Bevel: 2,
}

const StrokeCap = {
  Butt: 0,
  Round: 1,
  Square: 2,
}

const PathOp = {
  Difference: 0, // subtract the op path from the first path
  Intersect: 1, // intersect the two paths
  Union: 2, // union (inclusive-or) the two paths
  XOR: 3, // exclusive-or the two paths
  ReverseDifference: 4, // subtract the first path from the op path
}

const FillType = {
  Winding: 0,
  EvenOdd: 1,
  InverseWinding: 2,
  InverseEvenOdd: 3,
}

const SvgExportFlag = {
  ConvertTextToPaths: 0x01,
  NoPrettyXML: 0x02,
  RelativePathEncoding: 0x04,
}

const GlobalFontsSingleton = new GlobalFonts()
let FamilyNamesSet = JSON.parse(GlobalFontsSingleton._families)

// eslint-disable-next-line sonarjs/no-unused-collection
const Fonts = []

Object.defineProperty(GlobalFontsSingleton, 'register', {
  value: function register(fontData, nameAlias = '') {
    const result = GlobalFontsSingleton._register(fontData, nameAlias)
    FamilyNamesSet = JSON.parse(GlobalFontsSingleton._families)
    Fonts.push(fontData)
    return result
  },
  configurable: false,
  enumerable: false,
  writable: false,
})

Object.defineProperty(GlobalFontsSingleton, 'registerFromPath', {
  value: function registerFromPath(path, nameAlias = '') {
    const result = GlobalFontsSingleton._registerFromPath(path, nameAlias)
    FamilyNamesSet = JSON.parse(GlobalFontsSingleton._families)
    return result
  },
  configurable: false,
  enumerable: false,
  writable: false,
})

Object.defineProperty(GlobalFontsSingleton, 'loadFontsFromDir', {
  value: function loadFontsFromDir(path) {
    const result = GlobalFontsSingleton._loadFontsFromDir(path)
    FamilyNamesSet = JSON.parse(GlobalFontsSingleton._families)
    return result
  },
  configurable: false,
  enumerable: false,
  writable: false,
})

Object.defineProperty(GlobalFontsSingleton, 'families', {
  get: function () {
    return FamilyNamesSet
  },
})

Object.defineProperty(GlobalFontsSingleton, 'has', {
  value: function has(name) {
    return !!FamilyNamesSet.find(({ family }) => family === name)
  },
  configurable: false,
  enumerable: false,
  writable: false,
})

CanvasRenderingContext2D.prototype.createPattern = function createPattern(image, repetition) {
  if (image instanceof ImageData) {
    const pattern = new CanvasPattern(image, repetition, 0)
    Object.defineProperty(pattern, '_imageData', {
      writable: true,
      configurable: false,
      enumerable: false,
      value: null,
    })
    return pattern
  } else if (image instanceof Image) {
    return new CanvasPattern(image, repetition, 1)
  }
  throw TypeError('Image should be instance of ImageData or Image')
}

CanvasRenderingContext2D.prototype.getImageData = function getImageData(x, y, w, h) {
  const data = this._getImageData(x, y, w, h)
  return new ImageData(data, w, h)
}

Path2D.prototype.stroke = function stroke(strokeOptions = {}) {
  const width = typeof strokeOptions.width === 'undefined' ? 1 : strokeOptions.width
  const miterLimit = typeof strokeOptions.miterLimit === 'undefined' ? 4 : strokeOptions.miterLimit
  const join = typeof strokeOptions.join === 'undefined' ? StrokeJoin.Miter : strokeOptions.join
  const cap = typeof strokeOptions.cap === 'undefined' ? StrokeCap.Butt : strokeOptions.cap

  return this._stroke(width, miterLimit, join, cap)
}

Path2D.prototype.getFillTypeString = function getFillTypeString() {
  const fillType = this.getFillType()

  if (fillType === FillType.Winding) {
    return 'nonzero'
  } else if (fillType === FillType.EvenOdd) {
    return 'evenodd'
  } else {
    return 'nonzero' // default
  }
}

function createCanvas(width, height, flag) {
  const isSvgBackend = typeof flag !== 'undefined'
  const canvasElement = isSvgBackend ? new SVGCanvas(width, height) : new CanvasElement(width, height)
  const ctx = isSvgBackend
    ? new CanvasRenderingContext2D(width, height, GlobalFontsSingleton, flag)
    : new CanvasRenderingContext2D(width, height, GlobalFontsSingleton)

  // napi can not define writable: true but enumerable: false property
  Object.defineProperty(ctx, '_fillStyle', {
    value: '#000',
    configurable: false,
    enumerable: false,
    writable: true,
  })

  Object.defineProperty(ctx, '_strokeStyle', {
    value: '#000',
    configurable: false,
    enumerable: false,
    writable: true,
  })

  Object.defineProperty(ctx, 'createImageData', {
    value: function createImageData(widthOrImage, height) {
      if (widthOrImage instanceof ImageData) {
        return new ImageData(widthOrImage.width, widthOrImage.height)
      }
      return new ImageData(widthOrImage, height)
    },
    configurable: false,
    enumerable: false,
    writable: false,
  })

  Object.defineProperty(canvasElement, 'ctx', {
    value: ctx,
    enumerable: false,
    configurable: false,
  })

  ctx.canvas = canvasElement

  return canvasElement
}

if (!process.env.DISABLE_SYSTEM_FONTS_LOAD) {
  GlobalFontsSingleton.loadSystemFonts()
  FamilyNamesSet = JSON.parse(GlobalFontsSingleton._families)
}

function convertSVGTextToPath(input) {
  return _convertSVGTextToPath(Buffer.from(input), GlobalFontsSingleton)
}

module.exports = {
  createCanvas,
  Path2D,
  ImageData,
  Image,
  PathOp,
  FillType,
  StrokeCap,
  StrokeJoin,
  SvgExportFlag,
  ...Geometry,
  GlobalFonts: GlobalFontsSingleton,
  convertSVGTextToPath,
}
