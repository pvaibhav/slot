import type { CoreStates } from '@/api/client'

export const RESUME_STAMP = 'resume'

export interface StateGroup {
  id: string
  core: string
  resume: boolean
  states: { stamp: string; size: number }[]
}

export function groupStates(cores: CoreStates[]): StateGroup[] {
  return cores
    .filter(c => c.resume || c.states.length > 0)
    .map(c => ({
      id: c.id,
      core: c.core,
      resume: c.resume,
      states: [...c.states].sort((a, b) => b.stamp.localeCompare(a.stamp)),
    }))
}

export function stampLabel(stamp: string): string {
  const m = /^(\d{4}-\d{2}-\d{2})_(\d{2})-(\d{2})-(\d{2})$/.exec(stamp)
  return m ? `${m[1]} ${m[2]}:${m[3]}:${m[4]}` : stamp
}
