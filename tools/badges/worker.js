import kit from "./kit.json";

const REPO = "slot-cfw/slot";

function short(n) {
  if (n < 1000) return String(n);
  if (n < 100000) return (n / 1000).toFixed(1).replace(/\.0$/, "") + "k";
  if (n < 1000000) return Math.round(n / 1000) + "k";
  return (n / 1000000).toFixed(1).replace(/\.0$/, "") + "M";
}

function render(name, text) {
  let x = 0;
  let paths = "";
  for (let i = 0; i < text.length; i++) {
    const g = kit.glyphs[text[i]];
    paths += `<path transform="translate(${x.toFixed(2)} 0)" d="${g.d}"/>`;
    x += g.w;
    if (i < text.length - 1) x += kit.track;
  }
  return kit.badges[name]
    .replace("@@X@@", (kit.cx[name] - x / 2).toFixed(2))
    .replace("@@D@@", paths);
}

const QUERY = `query($owner: String!, $name: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    stargazerCount
    latestRelease { tagName }
    releases(first: 100, after: $after) {
      pageInfo { hasNextPage endCursor }
      nodes { releaseAssets(first: 100) { nodes { downloadCount } } }
    }
  }
}`;

async function graphql(after, env) {
  const [owner, name] = REPO.split("/");
  const res = await fetch("https://api.github.com/graphql", {
    method: "POST",
    headers: { authorization: `Bearer ${env.GITHUB_TOKEN}`, "user-agent": "slot-badges" },
    body: JSON.stringify({ query: QUERY, variables: { owner, name, after } }),
  });
  if (!res.ok) throw new Error(`graphql: ${res.status}`);
  const body = await res.json();
  if (body.errors) throw new Error(`graphql: ${body.errors[0].message}`);
  return body.data.repository;
}

async function refresh(env) {
  let repo;
  let after = null;
  let total = 0;
  do {
    repo = await graphql(after, env);
    for (const r of repo.releases.nodes) for (const a of r.releaseAssets.nodes) total += a.downloadCount;
    after = repo.releases.pageInfo.hasNextPage ? repo.releases.pageInfo.endCursor : null;
  } while (after);
  const values = {
    stars: short(repo.stargazerCount),
    downloads: short(total),
    release: repo.latestRelease.tagName,
  };
  for (const [name, text] of Object.entries(values)) {
    const key = `${name}.svg`;
    const head = await env.BADGES.head(key);
    if (head?.customMetadata?.text === text) continue;
    await env.BADGES.put(key, render(name, text), {
      httpMetadata: { contentType: "image/svg+xml" },
      customMetadata: { text },
    });
  }
}

export default {
  async scheduled(_event, env, ctx) {
    ctx.waitUntil(refresh(env));
  },
  async fetch(request, env) {
    const key = new URL(request.url).pathname.slice(1);
    if (!(key in { "stars.svg": 1, "downloads.svg": 1, "release.svg": 1 })) return new Response("not found", { status: 404 });
    let obj = await env.BADGES.get(key);
    if (!obj) {
      await refresh(env);
      obj = await env.BADGES.get(key);
    }
    return new Response(obj.body, {
      headers: {
        "content-type": "image/svg+xml",
        "cache-control": "public, max-age=60",
      },
    });
  },
};
