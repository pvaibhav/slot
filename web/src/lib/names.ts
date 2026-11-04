export function bareTitle(name: string): string {
  return name.replace(/\s*[([][^)\]]*[)\]]/g, '').trim() || name
}

export function shownTitles(names: string[]): Map<string, string> {
  const uses = new Map<string, number>()
  for (const name of names) {
    const bare = bareTitle(name)
    uses.set(bare, (uses.get(bare) ?? 0) + 1)
  }
  return new Map(names.map(name => {
    const bare = bareTitle(name)
    return [name, uses.get(bare) === 1 ? bare : name]
  }))
}
