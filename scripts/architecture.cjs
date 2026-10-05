function publicModules(source) {
  const modules = [...source.matchAll(/^pub mod (\w+)(?:;|\s*\{)/gm)].map((match) => match[1]);
  for (const match of source.matchAll(/^pub use [\w:]+::\{([^}]+)\};/gm)) {
    for (const item of match[1].split(",")) {
      const name = item.trim().split(/\s+as\s+/).pop();
      if (/^\w+$/.test(name)) modules.push(name);
    }
  }
  return [...new Set(modules)].sort();
}

function forbiddenEdges(packages) {
  const allowed = {
    "websh-core": [],
    "websh-site": ["websh-core"],
    "websh-cli": ["websh-core", "websh-site"],
    "websh-web": ["websh-core", "websh-site"],
  };
  const names = new Set(packages.map((pkg) => pkg.name));
  return [...new Set(packages.flatMap((pkg) => pkg.dependencies
    .filter((dep) => names.has(dep.name) && !allowed[pkg.name]?.includes(dep.name))
    .map((dep) => `${pkg.name}->${dep.name}`)))].sort();
}

module.exports = { publicModules, forbiddenEdges };
