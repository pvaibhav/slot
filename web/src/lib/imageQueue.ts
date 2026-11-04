const IN_FLIGHT = 3

let running = 0
const waiting: (() => void)[] = []

function next() {
  while (running < IN_FLIGHT && waiting.length) waiting.shift()!()
}

export function enqueue<T>(task: () => Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(new Error('Aborted'))
      return
    }
    const start = () => {
      signal.removeEventListener('abort', leave)
      running++
      task().then(resolve, reject).finally(() => {
        running--
        next()
      })
    }
    const leave = () => {
      waiting.splice(waiting.indexOf(start), 1)
      reject(new Error('Aborted'))
    }
    signal.addEventListener('abort', leave, { once: true })
    waiting.push(start)
    next()
  })
}
