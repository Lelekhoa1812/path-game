import { createGenerateRequest, isCurrentJob, normalizePuzzlePayload } from './protocol.js'

function createDefaultMapLoaderWorker() {
  return new Worker(new URL('./map-loader.worker.js', import.meta.url), { type: 'module' })
}

export class MapLoaderClient {
  constructor(createWorker = createDefaultMapLoaderWorker) {
    this.createWorker = createWorker
    this.worker = null
    this.activeJob = null
  }

  generate(options = {}) {
    if (this.activeJob) {
      this.cancel()
      // WASM generation is synchronous; if the worker cannot acknowledge promptly,
      // discard it so the next request starts on a fresh worker.
      if (this.activeJob?.cancelRequested) this.discardWorker()
    }

    if (!this.worker) this.createWorkerInstance()

    const request = createGenerateRequest(options)

    return new Promise((resolve, reject) => {
      this.activeJob = {
        jobId: request.jobId,
        resolve,
        reject,
        onProgress: typeof options.onProgress === 'function' ? options.onProgress : null,
        cancelRequested: false,
      }
      this.worker.postMessage(request)
    })
  }

  createWorkerInstance() {
    this.worker = this.createWorker()
    this.worker.onmessage = (event) => this.handleMessage(event.data)
    this.worker.onerror = (event) => this.handleWorkerFailure(event, 'Worker failed')
    this.worker.onmessageerror = (event) => this.handleWorkerFailure(event, 'Worker message failed')
  }

  cancel() {
    if (!this.worker || !this.activeJob) return

    const activeJob = this.activeJob
    if (activeJob.cancelRequested) return

    activeJob.cancelRequested = true
    this.worker.postMessage({ type: 'cancel', jobId: activeJob.jobId })
    activeJob.reject(new Error('Generation cancelled'))
  }

  handleMessage(message) {
    const activeJob = this.activeJob
    if (!activeJob || !isCurrentJob(message, activeJob.jobId)) return

    switch (message.type) {
      case 'progress':
        if (activeJob.onProgress) activeJob.onProgress(message)
        return
      case 'cancelled':
        this.finishActiveJob().reject(new Error('Generation cancelled'))
        return
      case 'result':
        this.finishActiveJob().resolve(normalizePuzzlePayload(message.puzzle))
        return
      case 'error':
        this.finishActiveJob().reject(new Error(message.message || 'Map generation failed'))
        return
      default:
        return
    }
  }

  handleWorkerFailure(event, fallbackMessage) {
    const activeJob = this.finishActiveJob()
    this.terminateWorker()

    if (!activeJob) return

    activeJob.reject(new Error(event?.message || fallbackMessage))
  }

  clearActiveJob() {
    this.activeJob = null
  }

  finishActiveJob() {
    const activeJob = this.activeJob
    this.clearActiveJob()
    return activeJob
  }

  terminateWorker() {
    if (this.worker) this.worker.terminate()
    this.worker = null
  }

  discardWorker() {
    this.clearActiveJob()
    this.terminateWorker()
  }

  terminate() {
    this.terminateWorker()
    this.clearActiveJob()
  }
}
