import '@testing-library/jest-dom'
// jsdom doesn't ship IndexedDB. fake-indexeddb provides an in-memory
// implementation we install globally for the test environment.
import 'fake-indexeddb/auto'

// jsdom 25 implements Blob/File but not Blob.prototype.text or
// Blob.prototype.arrayBuffer. The component under test reads uploaded
// files with `await file.text()` / `await file.arrayBuffer()`, so
// polyfill both via FileReader (which jsdom does ship).
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
if (typeof Blob !== 'undefined' && typeof Blob.prototype.arrayBuffer !== 'function') {
  Blob.prototype.arrayBuffer = function () {
    return new Promise<ArrayBuffer>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as ArrayBuffer)
      reader.onerror = () => reject(reader.error)
      reader.readAsArrayBuffer(this)
    })
  }
}
