#!/usr/bin/env python3
"""Create a dimhouse branch for every Ethereum devnet that lacks one.

A devnet's network name is <class>-<dir>: <class> is the repository
ethpandaops/<class>-devnets without "-devnets", and <dir> is a directory under
kubernetes/ there. Branch <network> holds what that devnet's dimhouse is built
from, and docker.yml publishes ethpandaops/dimhouse:<network> from it.

For each network without a branch, the Lighthouse image in the devnet's
ansible/inventories/<dir>/group_vars/all/images.yaml is resolved to a source
repository and ref through ethpandaops/eth-client-docker-image-builder, and the
dimhouse patch and overlay are applied to that source. When they apply, branch
<network> is pushed (this checkout's HEAD plus network.env and the patch) and
docker.yml is dispatched on it. Existing branches are never changed. Results
are printed and written to $GITHUB_STEP_SUMMARY.

Usage: scripts/devnet-branches.py [--dry-run] [--network NAME ...]
Needs git, PyYAML, and a GitHub token in GH_TOKEN or GITHUB_TOKEN.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

import yaml

ORG = "ethpandaops"
EXCLUDED_REPOS = {"template-devnets"}
BUILDER = "ethpandaops/eth-client-docker-image-builder"
ROOT = Path(__file__).resolve().parent.parent
REPO = os.environ.get("GITHUB_REPOSITORY", "ethpandaops/dimhouse")
TOKEN = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")


class Skip(Exception):
    """The network gets no branch, for the reason in the message."""


def api(path, method="GET", body=None, raw=False):
    """Calls the GitHub API. Returns None on 404; lists follow pagination."""
    url = path if path.startswith("https://") else f"https://api.github.com{path}"
    results = []
    while url:
        req = urllib.request.Request(url, method=method, data=json.dumps(body).encode() if body else None)
        req.add_header("Accept", "application/vnd.github.raw" if raw else "application/vnd.github+json")
        if TOKEN:
            req.add_header("Authorization", f"Bearer {TOKEN}")
        try:
            with urllib.request.urlopen(req) as resp:
                data = resp.read()
                link = resp.headers.get("Link") or ""
        except urllib.error.HTTPError as e:
            if e.code == 404 and method == "GET":
                return None
            raise RuntimeError(f"{method} {url}: HTTP {e.code} {e.read().decode(errors='replace')}") from e
        if raw:
            return data.decode()
        if not data:
            return None
        page = json.loads(data)
        if not isinstance(page, list):
            return page
        results.extend(page)
        nxt = re.search(r'<([^>]+)>;\s*rel="next"', link)
        url = nxt.group(1) if nxt else None
    return results


def git(*args, cwd=None, check=True):
    return subprocess.run(["git", *args], cwd=cwd, check=check, text=True, capture_output=True)


def discover():
    """Yields (network, devnet repo, dir) for every devnet under kubernetes/."""
    repos = api(f"/orgs/{ORG}/repos?per_page=100&type=all")
    for repo in sorted(repos, key=lambda r: r["name"]):
        name = repo["name"]
        if not name.endswith("-devnets") or repo["archived"] or name in EXCLUDED_REPOS:
            continue
        entries = api(f"/repos/{ORG}/{name}/contents/kubernetes") or []
        for entry in sorted(entries, key=lambda e: e["name"]):
            if entry["type"] == "dir":
                yield f"{name.removesuffix('-devnets')}-{entry['name']}", name, entry["name"]


def lighthouse_image(repo, dirname):
    path = f"ansible/inventories/{dirname}/group_vars/all/images.yaml"
    text = api(f"/repos/{ORG}/{repo}/contents/{path}", raw=True)
    if text is None:
        raise Skip(f"no {path}")
    images = (yaml.safe_load(text) or {}).get("default_ethereum_client_images") or {}
    if not images.get("lighthouse"):
        raise Skip("runs no Lighthouse")
    return images["lighthouse"]


def builder_lighthouse_configs(work):
    """Maps each ethpandaops/lighthouse tag the image builder publishes to its build config."""
    src = work / "eth-client-docker-image-builder"
    git("clone", "--quiet", "--depth", "1", f"https://github.com/{BUILDER}", str(src))
    subprocess.run([sys.executable, "generate_config.py"], cwd=src, check=True, capture_output=True)
    configs = yaml.safe_load((src / "config.yaml").read_text())
    return {c["target"]["tag"]: c for c in configs if c["target"]["repository"] == f"{ORG}/lighthouse"}


def resolve(image, builder):
    """Resolves a Lighthouse image to (source repo, ref, commit or None, notes)."""
    name, _, tag = image.removeprefix("docker.io/").rpartition(":")
    notes = []
    if name == f"{ORG}/lighthouse":
        commit = None
        config = builder.get(tag)
        # The builder also publishes every build as <tag>-<short commit>.
        pinned = re.fullmatch(r"(.+)-([0-9a-f]{7,40})", tag)
        if config is None and pinned and pinned.group(1) in builder:
            config, commit = builder[pinned.group(1)], pinned.group(2)
        if config is None:
            raise Skip(f"{BUILDER} does not build {image}")
        source = config["source"]
        if source.get("patch"):
            raise Skip(f"{BUILDER} patches the source of {image} with {source['patch']}")
        repo, ref = source["repository"], source["ref"]
        dockerfile = config["target"].get("dockerfile")
        if dockerfile and dockerfile != "./lighthouse/Dockerfile":
            notes.append(f"devnet image is built with {BUILDER} {dockerfile}; dimhouse uses its own Dockerfile")
    elif name == "sigp/lighthouse":
        # Upstream images are tagged with the release they were built from.
        repo, ref, commit = "sigp/lighthouse", tag, None
    else:
        raise Skip(f"no known source for {image}")
    if not git("ls-remote", "--exit-code", f"https://github.com/{repo}", f"refs/heads/{ref}", f"refs/tags/{ref}", check=False).stdout:
        raise Skip(f"{repo} has no branch or tag {ref}")
    if commit:
        found = api(f"/repos/{repo}/commits/{commit}")
        if found is None:
            raise Skip(f"{repo} has no commit {commit}")
        commit = found["sha"]
    return repo, ref, commit, notes


def patch_candidates(repo, ref):
    """Patches to try on <repo>@<ref>: its own first, then the closest kind of base."""
    exact = Path("patches", repo, f"{ref}.patch")
    others = [p.relative_to(ROOT) for p in sorted((ROOT / "patches").rglob("*.patch"))]
    others = [p for p in others if p != exact]
    natural = lambda p: [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", p.stem)]
    unstable = [p for p in others if p.stem == "unstable"]
    releases = sorted((p for p in others if p.stem != "unstable"), key=natural, reverse=True)
    # A release tag is closest to the newest release patches; a devnet branch to unstable.
    ordered = releases + unstable if re.match(r"v\d", ref) else unstable + releases
    return ([exact] if (ROOT / exact).exists() else []) + ordered


def apply_patch(network, repo, ref, commit, work):
    """Applies the first patch candidate that fits. Returns (dimhouse worktree, patch, source commit)."""
    lighthouse = work / "lighthouse"
    git("init", "--quiet", str(lighthouse))
    git("fetch", "--quiet", "--depth", "1", f"https://github.com/{repo}", commit or ref, cwd=lighthouse)
    git("checkout", "--quiet", "FETCH_HEAD", cwd=lighthouse)
    source_commit = git("rev-parse", "HEAD", cwd=lighthouse).stdout.strip()

    tree = work / "dimhouse"
    git("worktree", "add", "--quiet", "--detach", str(tree), "HEAD", cwd=ROOT)
    target = tree / "patches" / repo / f"{ref}.patch"
    tried = []
    for candidate in patch_candidates(repo, ref):
        git("reset", "--quiet", "--hard", cwd=lighthouse)
        git("clean", "--quiet", "-fdx", cwd=lighthouse)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT / candidate).read_bytes())
        result = subprocess.run(
            ["bash", str(tree / "scripts/apply-dimhouse-patch.sh"), repo, ref, str(lighthouse)],
            capture_output=True, text=True,
        )
        if result.returncode == 0:
            return tree, candidate, source_commit
        tried.append(str(candidate))
    raise Skip(f"no patch applies to {repo}@{source_commit[:10]} (tried {', '.join(tried)})")


def create_branch(network, tree, repo, ref, commit, image, devnet_repo):
    (tree / "network.env").write_text(
        f"# Lighthouse source of the {network} dimhouse image, read by .github/workflows/docker.yml.\n"
        f"# The devnet runs {image} ({ORG}/{devnet_repo}).\n"
        f"NETWORK={network}\n"
        f"LIGHTHOUSE_REPO={repo}\n"
        f"LIGHTHOUSE_REF={ref}\n"
        f"LIGHTHOUSE_COMMIT={commit or ''}\n"
    )
    git("add", "network.env", "patches", cwd=tree)
    git(
        "-c", "user.name=github-actions[bot]",
        "-c", "user.email=41898282+github-actions[bot]@users.noreply.github.com",
        "commit", "--quiet", "-m", f"build: {network} from {repo}@{ref}",
        "-m", f"The {network} devnet runs {image}.",
        cwd=tree,
    )
    # A plain push only creates the branch or fast-forwards it; check it is still absent.
    if git("ls-remote", "--exit-code", "--heads", "origin", f"refs/heads/{network}", cwd=ROOT, check=False).returncode == 0:
        raise Skip(f"branch {network} appeared while preparing it")
    git("push", "--quiet", "origin", f"HEAD:refs/heads/{network}", cwd=tree)
    # Pushes made with the workflow's token start no workflows, so start the build.
    try:
        api(f"/repos/{REPO}/actions/workflows/docker.yml/dispatches", method="POST", body={"ref": network})
    except RuntimeError as e:
        return f"created branch; dispatching docker.yml failed: {e}"
    return "created branch; docker.yml dispatched"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dry-run", action="store_true", help="report what would be created without pushing")
    parser.add_argument("--network", action="append", help="only consider these networks")
    args = parser.parse_args()

    branches = {
        line.split("refs/heads/", 1)[1]
        for line in git("ls-remote", "--heads", "origin", cwd=ROOT).stdout.splitlines()
    }
    rows = []
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        builder = builder_lighthouse_configs(work)
        for network, devnet_repo, dirname in discover():
            if args.network and network not in args.network:
                continue
            row = {"network": network, "image": "", "source": "", "patch": "", "result": ""}
            rows.append(row)
            net_work = work / network
            net_work.mkdir()
            try:
                row["image"] = image = lighthouse_image(devnet_repo, dirname)
                if network in branches:
                    row["result"] = "branch exists, left unchanged"
                    continue
                repo, ref, commit, notes = resolve(image, builder)
                row["source"] = f"{repo}@{ref}" + (f" ({commit[:10]})" if commit else "")
                tree, patch, source_commit = apply_patch(network, repo, ref, commit, net_work)
                if not commit:
                    row["source"] += f" (head {source_commit[:10]})"
                row["patch"] = str(patch)
                if args.dry_run:
                    row["result"] = "patch applies; would create branch"
                else:
                    row["result"] = create_branch(network, tree, repo, ref, commit, image, devnet_repo)
                row["result"] += "".join(f"; note: {n}" for n in notes)
            except Skip as e:
                row["result"] = f"no branch: {e}"
            finally:
                if (net_work / "dimhouse").exists():
                    git("worktree", "remove", "--force", str(net_work / "dimhouse"), cwd=ROOT)

    report(rows, args.dry_run)


def report(rows, dry_run):
    lines = [
        f"## Devnet branches{' (dry run)' if dry_run else ''}",
        "",
        "| Network | Lighthouse image | Source | Patch | Result |",
        "|---|---|---|---|---|",
    ]
    code = lambda s: f"`{s}`" if s else ""
    lines += [
        f"| {code(r['network'])} | {code(r['image'])} | {r['source']} | {code(r['patch'])} | {r['result']} |"
        for r in rows
    ]
    failed = [r for r in rows if r["result"].startswith("no branch: no patch applies")]
    if failed:
        lines += [
            "",
            "### Needs a rebase",
            "",
            "No dimhouse patch applies to the Lighthouse source of these devnets, so they have no branch yet.",
            "Rebase a patch onto the source (README: Fixing a patch conflict), save it as",
            "`patches/<org>/<repo>/<ref>.patch` on master, and the next run creates the branch:",
            "",
        ]
        lines += [f"- `{r['network']}`: {r['source']}" for r in failed]
    text = "\n".join(lines) + "\n"
    print(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as f:
            f.write(text)
    for r in failed:
        print(f"::warning::{r['network']}: {r['result']}")


if __name__ == "__main__":
    main()
