import '@testing-library/jest-dom'

// jsdom 25 implements Blob/File but not Blob.prototype.text. The component
// under test reads uploaded files with `await file.text()`, so polyfill it
// here via FileReader (which jsdom does ship).
if (typeof Blob !== 'undefined' && typeof Blob.prototype.text !== 'function') {
  Blob.prototype.text = function () {
    return new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as string)
      reader.onerror = () => reject(reader.error)
      reader.readAsText(this)
    })
  }
}
