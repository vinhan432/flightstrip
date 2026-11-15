// Smoke test for the replay page, without a browser.
//
//     node web/smoke.mjs
//
// It boots the static server, stubs the handful of DOM APIs app.js touches, runs
// the real app.js against the real fixtures, and asserts that the page filled
// itself in without throwing. That is not a visual check — it cannot see layout —
// but it does catch the failure that matters when there is no bundler: a client
// that throws on load and leaves a blank page.

import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const here = path.dirname(fileURLToPath(import.meta.url));
const port = 4399;
// Relative URLs resolve against the page, exactly as they do in a browser.
const base = `http://localhost:${port}/web/index.html`;

function element(tag = 'div') {
  return {
    tagName: String(tag).toUpperCase(),
    children: [],
    attributes: {},
    style: {},
    dataset: {},
    classList: { toggle() {}, add() {}, remove() {}, contains: () => false },
    setAttribute(key, value) {
      this.attributes[key] = String(value);
    },
    getAttribute(key) {
      return this.attributes[key];
    },
    append(...nodes) {
      this.children.push(...nodes);
    },
    appendChild(node) {
      this.children.push(node);
      return node;
    },
    listeners: {},
    addEventListener(type, handler) {
      (this.listeners[type] ||= []).push(handler);
    },
    dispatch(type) {
      for (const handler of this.listeners[type] || []) handler({ preventDefault() {} });
    },
    removeEventListener() {},
    querySelectorAll: () => [],
    scrollIntoView() {},
    click() {},
    focus() {},
    innerHTML: '',
    textContent: '',
    value: '',
    files: [],
    clientWidth: 960,
  };
}

function stubDocument() {
  const byId = new Map();
  return {
    byId,
    document: {
      getElementById(id) {
        if (!byId.has(id)) byId.set(id, element());
        return byId.get(id);
      },
      createElement: (tag) => element(tag),
      createElementNS: (_ns, tag) => element(tag),
      querySelectorAll: () => [],
      addEventListener() {},
    },
  };
}

function startServer() {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [path.join(here, 'serve.js')], {
      env: { ...process.env, PORT: String(port) },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let output = '';
    const timer = setTimeout(() => reject(new Error(`server did not start: ${output}`)), 8000);
    child.stdout.on('data', (chunk) => {
      output += chunk.toString();
      if (output.includes('Flightstrip replay')) {
        clearTimeout(timer);
        resolve(child);
      }
    });
    child.stderr.on('data', (chunk) => {
      output += chunk.toString();
    });
    child.on('exit', (code) => {
      clearTimeout(timer);
      reject(new Error(`server exited with ${code}: ${output}`));
    });
  });
}

const failures = [];

function check(condition, message) {
  if (condition) {
    console.log(`  ok   ${message}`);
  } else {
    console.log(`  fail ${message}`);
    failures.push(message);
  }
}

async function main() {
  const server = await startServer();
  try {
    const { document, byId } = stubDocument();
    const fetched = [];
    const realFetch = globalThis.fetch;

    const sandbox = {
      document,
      console,
      setTimeout,
      clearTimeout,
      requestAnimationFrame: () => 0,
      fetch: (resource, options) => {
        fetched.push(String(resource));
        return realFetch(new URL(String(resource), base), options);
      },
      URL,
      JSON,
      Number,
      Math,
      String,
      Object,
      Array,
      Error,
    };
    sandbox.window = {
      requestAnimationFrame: () => 0,
      addEventListener() {},
      cancelAnimationFrame() {},
    };
    sandbox.globalThis = sandbox;

    const source = await fs.readFile(path.join(here, 'app.js'), 'utf8');
    vm.createContext(sandbox);
    vm.runInContext(source, sandbox, { filename: 'app.js' });

    // Let the catalogue fetch and the first session fetch settle.
    await new Promise((resolve) => setTimeout(resolve, 800));

    const title = byId.get('session-title');
    const steps = byId.get('steps');
    const files = byId.get('files-body');
    const verdict = byId.get('verdict');
    const tape = byId.get('tape');

    check(fetched.includes('sessions.json'), 'the page fetches its session catalogue');
    check(
      fetched.some((url) => url.includes('fixtures/')),
      'the page fetches a fixture session',
    );
    const meta = byId.get('session-meta');
    check(
      title && title.textContent === 'Checkout: split the store',
      `title is "${title && title.textContent}"${meta && meta.textContent ? ` (${meta.textContent})` : ''}`,
    );
    check(steps && steps.children.length > 10, `step rows rendered (${steps ? steps.children.length : 0})`);
    check(files && files.children.length === 3, `file rows rendered (${files ? files.children.length : 0})`);
    check(verdict && verdict.children.length >= 1, 'verdict chips rendered');
    check(tape && tape.children.length === 1, 'the strip chart was drawn');

    const svg = tape.children[0];
    check(svg && svg.children.length > 20, `tape has ${svg ? svg.children.length : 0} drawn nodes`);
    check(
      svg && svg.children.some((node) => node.tagName === 'G' && node.children.length >= 2),
      'tape contains interactive step marks',
    );

    // Selecting a step must produce a detail panel; call it through the row.
    const row = steps.children.find((item) => item.children.length > 0);
    const button = row && row.children[0];
    check(button && button.innerHTML.includes('step-time'), 'step rows carry their markup');

    const fileRow = steps.children.find((item) =>
      item.children[0] ? item.children[0].innerHTML.includes('class="chip file"') : false,
    );
    check(Boolean(fileRow), 'a file-change row is present');
    if (fileRow) {
      fileRow.children[0].dispatch('click');
      const detail = byId.get('detail');
      check(detail.innerHTML.includes('class="hunk"'), 'clicking a file change renders a diff hunk');
      check(detail.innerHTML.includes('class="path"'), 'the detail names the file');
    }
  } finally {
    server.kill();
  }

  if (failures.length > 0) {
    console.error(`\n${failures.length} check(s) failed`);
    process.exit(1);
  }
  console.log('\nsmoke test passed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
