import init, { generate_puzzle } from '../../public/map_loader_wasm/map_loader_wasm.js'

let wasmReady = null
const cancelledJobIds = new Set()

async function ensureWasm() {
  if (!wasmReady) wasmReady = init()
  await wasmReady
}

self.onmessage = async (event) => {
  const message = event.data

  if (message.type === 'cancel') {
    cancelledJobIds.add(message.jobId)
    self.postMessage({ type: 'cancelled', jobId: message.jobId })
    return
  }

  if (message.type !== 'generate') return

  try {
    await ensureWasm()
    if (cancelledJobIds.has(message.jobId)) return

    self.postMessage({ type: 'progress', jobId: message.jobId, stage: 'Building map' })

    const puzzle = generate_puzzle(message)
    if (cancelledJobIds.has(message.jobId)) return

    self.postMessage({ type: 'result', jobId: message.jobId, puzzle })
  } catch (error) {
    self.postMessage({
      type: 'error',
      jobId: message.jobId,
      message: error && error.message ? error.message : String(error),
    })
  } finally {
    cancelledJobIds.delete(message.jobId)
  }
}
