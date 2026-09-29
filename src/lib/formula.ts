/**
 * A small spreadsheet formula engine covering the functions a lawyer actually
 * uses for damages tables, fee schedules, and discovery tallies.
 *
 * Design notes:
 * - Formulas are parsed once into an AST and evaluated with cycle detection, so
 *   a self-referential cell reports an error instead of hanging the UI.
 * - Dependency tracking is done by evaluation rather than a static graph: the
 *   grid recomputes cells in a topological order derived from the AST, which
 *   keeps recalculation correct after row/column insertion and deletion.
 */

export interface Cell {
  /** Displayed value. A formula cell always carries its computed result. */
  v?: string;
  /** Formula source, including the leading '='. */
  f?: string;
}

export type Grid = Record<string, Cell>;

export const COLS = 26;

export function colName(index: number): string {
  let s = '';
  let n = index;
  while (n >= 0) {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
  }
  return s;
}

export function colIndex(name: string): number {
  let n = 0;
  for (const ch of name.toUpperCase()) {
    n = n * 26 + (ch.charCodeAt(0) - 64);
  }
  return n - 1;
}

export function cellKey(row: number, col: number): string {
  return `${row},${col}`;
}

export function parseKey(key: string): { row: number; col: number } {
  const [r, c] = key.split(',').map(Number);
  return { row: r, col: c };
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

type TokenType =
  | 'number'
  | 'string'
  | 'ident'
  | 'ref'
  | 'op'
  | 'lparen'
  | 'rparen'
  | 'comma'
  | 'colon';

interface Token {
  type: TokenType;
  value: string;
}

function tokenize(src: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < src.length) {
    const ch = src[i];

    if (/\s/.test(ch)) {
      i++;
      continue;
    }

    if (ch === '"') {
      let value = '';
      i++;
      while (i < src.length && src[i] !== '"') {
        if (src[i] === '\\' && i + 1 < src.length) {
          value += src[i + 1];
          i += 2;
        } else {
          value += src[i];
          i++;
        }
      }
      if (src[i] === '"') i++;
      tokens.push({ type: 'string', value });
      continue;
    }

    if (/[0-9]/.test(ch) || (ch === '.' && /[0-9]/.test(src[i + 1] ?? ''))) {
      let value = '';
      while (i < src.length && /[0-9.]/.test(src[i])) {
        value += src[i];
        i++;
      }
      tokens.push({ type: 'number', value });
      continue;
    }

    // A cell reference is a letter run followed by digits, optionally with a
    // $ absolute marker on either part.
    const refMatch = /^\$?[A-Za-z]{1,3}\$?[0-9]{1,7}/.exec(src.slice(i));
    // A function name is letters/underscore not followed by '(' as a ref... it
    // is followed by '(', which the ref pattern cannot match.
    if (refMatch) {
      tokens.push({ type: 'ref', value: refMatch[0] });
      i += refMatch[0].length;
      continue;
    }

    const identMatch = /^[A-Za-z_][A-Za-z0-9_.]*/.exec(src.slice(i));
    if (identMatch) {
      tokens.push({ type: 'ident', value: identMatch[0].toUpperCase() });
      i += identMatch[0].length;
      continue;
    }

    if (ch === '(') {
      tokens.push({ type: 'lparen', value: ch });
      i++;
      continue;
    }
    if (ch === ')') {
      tokens.push({ type: 'rparen', value: ch });
      i++;
      continue;
    }
    if (ch === ',' || ch === ';') {
      tokens.push({ type: 'comma', value: ',' });
      i++;
      continue;
    }
    if (ch === ':') {
      tokens.push({ type: 'colon', value: ch });
      i++;
      continue;
    }

    const two = src.slice(i, i + 2);
    if (['>=', '<=', '<>'].includes(two)) {
      tokens.push({ type: 'op', value: two });
      i += 2;
      continue;
    }

    if ('+-*/^%<>=&'.includes(ch)) {
      tokens.push({ type: 'op', value: ch });
      i++;
      continue;
    }

    throw new FormulaError(`Unexpected character '${ch}'`);
  }

  return tokens;
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

type Node =
  | { kind: 'num'; value: number }
  | { kind: 'str'; value: string }
  | { kind: 'bool'; value: boolean }
  | { kind: 'ref'; key: string }
  | { kind: 'range'; start: string; end: string }
  | { kind: 'bin'; op: string; left: Node; right: Node }
  | { kind: 'unary'; op: string; operand: Node }
  | { kind: 'call'; name: string; args: Node[] };

/** Spreadsheet-style tokens shown in the cell, so errors read at a glance. */
export type ErrorCode =
  | '#DIV/0!'
  | '#NAME?'
  | '#VALUE!'
  | '#NUM!'
  | '#REF!'
  | '#CIRCULAR!'
  | '#N/A'
  | '#ERROR!';

export class FormulaError extends Error {
  readonly code: ErrorCode;
  constructor(message: string, code: ErrorCode = '#ERROR!') {
    super(message);
    this.code = code;
  }
}

class Parser {
  private pos = 0;
  constructor(private tokens: Token[]) {}

  parse(): Node {
    const node = this.parseComparison();
    if (this.pos < this.tokens.length) {
      throw new FormulaError(`Unexpected token '${this.tokens[this.pos].value}'`);
    }
    return node;
  }

  private peek(): Token | undefined {
    return this.tokens[this.pos];
  }

  private parseComparison(): Node {
    let left = this.parseConcat();
    for (;;) {
      const t = this.peek();
      if (t && t.type === 'op' && ['=', '<', '>', '<=', '>=', '<>'].includes(t.value)) {
        this.pos++;
        left = { kind: 'bin', op: t.value, left, right: this.parseConcat() };
      } else return left;
    }
  }

  private parseConcat(): Node {
    let left = this.parseAdditive();
    while (this.peek()?.type === 'op' && this.peek()!.value === '&') {
      this.pos++;
      left = { kind: 'bin', op: '&', left, right: this.parseAdditive() };
    }
    return left;
  }

  private parseAdditive(): Node {
    let left = this.parseMultiplicative();
    for (;;) {
      const t = this.peek();
      if (t && t.type === 'op' && ['+', '-'].includes(t.value)) {
        this.pos++;
        left = { kind: 'bin', op: t.value, left, right: this.parseMultiplicative() };
      } else return left;
    }
  }

  private parseMultiplicative(): Node {
    let left = this.parseUnary();
    for (;;) {
      const t = this.peek();
      if (t && t.type === 'op' && ['*', '/', '%'].includes(t.value)) {
        this.pos++;
        left = { kind: 'bin', op: t.value, left, right: this.parseUnary() };
      } else return left;
    }
  }

  private parseUnary(): Node {
    const t = this.peek();
    if (t && t.type === 'op' && (t.value === '-' || t.value === '+')) {
      this.pos++;
      return { kind: 'unary', op: t.value, operand: this.parseUnary() };
    }
    return this.parsePower();
  }

  private parsePower(): Node {
    const base = this.parsePrimary();
    if (this.peek()?.type === 'op' && this.peek()!.value === '^') {
      this.pos++;
      return { kind: 'bin', op: '^', left: base, right: this.parseUnary() };
    }
    return base;
  }

  private parsePrimary(): Node {
    const t = this.peek();
    if (!t) throw new FormulaError('Unexpected end of formula');

    if (t.type === 'number') {
      this.pos++;
      return { kind: 'num', value: parseFloat(t.value) };
    }
    if (t.type === 'string') {
      this.pos++;
      return { kind: 'str', value: t.value };
    }
    if (t.type === 'lparen') {
      this.pos++;
      const inner = this.parseComparison();
      if (this.peek()?.type !== 'rparen') throw new FormulaError('Expected )');
      this.pos++;
      return inner;
    }
    if (t.type === 'ref') {
      this.pos++;
      if (this.peek()?.type === 'colon') {
        this.pos++;
        const end = this.peek();
        if (!end || end.type !== 'ref') throw new FormulaError('Expected range end');
        this.pos++;
        return { kind: 'range', start: t.value, end: end.value };
      }
      return { kind: 'ref', key: normalizeRef(t.value) };
    }
    if (t.type === 'ident') {
      const name = t.value;
      this.pos++;
      if (name === 'TRUE') return { kind: 'bool', value: true };
      if (name === 'FALSE') return { kind: 'bool', value: false };
      if (this.peek()?.type === 'lparen') {
        this.pos++;
        const args: Node[] = [];
        if (this.peek()?.type !== 'rparen') {
          for (;;) {
            args.push(this.parseComparison());
            if (this.peek()?.type === 'comma') {
              this.pos++;
              continue;
            }
            break;
          }
        }
        if (this.peek()?.type !== 'rparen') throw new FormulaError(`Expected ) in ${name}`);
        this.pos++;
        return { kind: 'call', name, args };
      }
      // A bare identifier used as text, e.g. =Draft
      return { kind: 'str', value: name.toLowerCase() };
    }

    throw new FormulaError(`Unexpected token '${t.value}'`);
  }
}

/** Strips '$' markers and normalises to zero-based row/col. */
function normalizeRef(ref: string): string {
  const m = /^\$?([A-Za-z]{1,3})\$?([0-9]{1,7})$/.exec(ref);
  if (!m) throw new FormulaError(`Bad reference '${ref}'`, '#REF!');
  return cellKey(parseInt(m[2], 10) - 1, colIndex(m[1]));
}

const astCache = new Map<string, Node>();

export function parseFormula(formula: string): Node {
  const cached = astCache.get(formula);
  if (cached) return cached;
  const src = formula.startsWith('=') ? formula.slice(1) : formula;
  const node = new Parser(tokenize(src)).parse();
  // Bound the cache; sheets are small but a long session can churn formulas.
  if (astCache.size > 5000) astCache.clear();
  astCache.set(formula, node);
  return node;
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

export type Scalar = number | string | boolean;
export type Value = Scalar | Scalar[];

class EvalError extends FormulaError {}

/** Reads a cell's scalar value, preferring the cached result over raw text. */
export type CellReader = (key: string) => Value;

function toNumber(v: Value): number {
  if (typeof v === 'number') return v;
  if (typeof v === 'boolean') return v ? 1 : 0;
  if (v === '' || v == null) return 0;
  const n = parseFloat(String(v).replace(/[$,\s]/g, ''));
  if (Number.isNaN(n)) throw new EvalError(`'${v}' is not a number`, '#VALUE!');
  return n;
}

function toText(v: Value): string {
  if (typeof v === 'boolean') return v ? 'TRUE' : 'FALSE';
  if (Array.isArray(v)) return v.map(toText).join(', ');
  if (v === '' || v == null) return '';
  return String(v);
}

function compare(a: Value, b: Value): number {
  const an = typeof a === 'number' ? a : parseFloat(String(a));
  const bn = typeof b === 'number' ? b : parseFloat(String(b));
  if (!Number.isNaN(an) && !Number.isNaN(bn)) return an < bn ? -1 : an > bn ? 1 : 0;
  const as = toText(a).toLowerCase();
  const bs = toText(b).toLowerCase();
  return as < bs ? -1 : as > bs ? 1 : 0;
}

/**
 * Flattens nested range results to a flat list of scalars.
 *
 * Iterative and order-preserving: SUMPRODUCT pairs elements by position, so a
 * reversed or short-circuited result would total the wrong rows.
 */
function flatten(values: Value[]): Scalar[] {
  const out: Scalar[] = [];
  const pending: Value[] = [...values];
  for (let i = 0; i < pending.length; i++) {
    const v = pending[i];
    if (Array.isArray(v)) {
      for (const inner of v) pending.push(inner);
    } else {
      out.push(v);
    }
  }
  return out;
}

/** Numeric values from a range/array, skipping blanks and non-numeric text. */
function numericOnly(values: Value[]): number[] {
  const out: number[] = [];
  for (const v of flatten(values)) {
    if (typeof v === 'number') out.push(v);
    else if (typeof v === 'boolean') out.push(v ? 1 : 0);
    else if (typeof v === 'string' && v.trim() !== '') {
      const n = parseFloat(v.replace(/[$,\s]/g, ''));
      if (!Number.isNaN(n)) out.push(n);
    }
  }
  return out;
}

/**
 * Coercion for SUMPRODUCT, where blanks and non-numeric text count as zero
 * instead of raising, so a sparse damages table still totals correctly.
 */
function numericOrZero(v: Scalar): number {
  if (typeof v === 'number') return Number.isFinite(v) ? v : 0;
  if (typeof v === 'boolean') return v ? 1 : 0;
  if (v === '' || v == null) return 0;
  const n = parseFloat(String(v).replace(/[$,\s]/g, ''));
  return Number.isNaN(n) ? 0 : n;
}

function rangeKeys(startRef: string, endRef: string): string[] {
  const m1 = /^\$?([A-Za-z]{1,3})\$?([0-9]+)$/.exec(startRef);
  const m2 = /^\$?([A-Za-z]{1,3})\$?([0-9]+)$/.exec(endRef);
  if (!m1 || !m2) throw new FormulaError(`Bad range ${startRef}:${endRef}`, '#REF!');
  const r1 = parseInt(m1[2], 10) - 1;
  const r2 = parseInt(m2[2], 10) - 1;
  const c1 = colIndex(m1[1]);
  const c2 = colIndex(m2[1]);
  const keys: string[] = [];
  for (let r = Math.min(r1, r2); r <= Math.max(r1, r2); r++) {
    for (let c = Math.min(c1, c2); c <= Math.max(c1, c2); c++) {
      keys.push(cellKey(r, c));
    }
  }
  return keys;
}

function evaluateNode(node: Node, read: CellReader, seen: Set<string>): Value {
  switch (node.kind) {
    case 'num':
      return node.value;
    case 'str':
      return node.value;
    case 'bool':
      return node.value;
    case 'ref': {
      // `seen` is the current evaluation *path*, not a set of every cell ever
      // touched: it is added before descending and removed on the way out, so
      // repeating a reference (=A1/A1, =IF(A1=0,1/A1,5)) is legal while a true
      // self-reference still trips the cycle check.
      if (seen.has(node.key)) throw new EvalError('Circular reference', '#CIRCULAR!');
      seen.add(node.key);
      try {
        return read(node.key);
      } finally {
        seen.delete(node.key);
      }
    }
    case 'range':
      // Range reads are independent of each other, so they do not participate
      // in cycle detection; only single-cell refs need a fresh `seen` set.
      return rangeKeys(node.start, node.end).map((k) => {
        if (seen.has(k)) return '';
        // A cell inside a range can itself hold a range result; flatten it so
        // the outer caller always sees a flat list of scalars.
        return flatten([read(k)])[0] ?? '';
      });
    case 'unary': {
      const v = evaluateNode(node.operand, read, seen);
      return node.op === '-' ? -toNumber(v) : toNumber(v);
    }
    case 'bin': {
      const l = evaluateNode(node.left, read, seen);
      const r = evaluateNode(node.right, read, seen);
      switch (node.op) {
        case '+':
          return toNumber(l) + toNumber(r);
        case '-':
          return toNumber(l) - toNumber(r);
        case '*':
          return toNumber(l) * toNumber(r);
        case '/': {
          const d = toNumber(r);
          if (d === 0) throw new EvalError('Division by zero', '#DIV/0!');
          return toNumber(l) / d;
        }
        case '%':
          return toNumber(l) % toNumber(r);
        case '^':
          return toNumber(l) ** toNumber(r);
        case '&':
          return toText(l) + toText(r);
        case '=':
          return compare(l, r) === 0;
        case '<>':
          return compare(l, r) !== 0;
        case '<':
          return compare(l, r) < 0;
        case '>':
          return compare(l, r) > 0;
        case '<=':
          return compare(l, r) <= 0;
        case '>=':
          return compare(l, r) >= 0;
        default:
          throw new FormulaError(`Unknown operator ${node.op}`);
      }
    }
    case 'call':
      return callFunction(node.name, node.args, read, seen);
  }
}

function callFunction(name: string, argNodes: Node[], read: CellReader, seen: Set<string>): Value {
  // Arguments are evaluated lazily per function: IF must not evaluate the branch
  // it does not take, or IF(A1=0, 1/A1, 0) would always error.
  const lazy = (...idx: number[]) =>
    idx.map((i) => (argNodes[i] ? evaluateNode(argNodes[i], read, seen) : 0));

  switch (name) {
    case 'SUM':
      return numericOnly(argNodes.map((a) => evaluateNode(a, read, seen))).reduce(
        (a, b) => a + b,
        0,
      );
    case 'PRODUCT':
      return numericOnly(argNodes.map((a) => evaluateNode(a, read, seen))).reduce(
        (a, b) => a * b,
        1,
      );
    case 'AVERAGE': {
      const nums = numericOnly(argNodes.map((a) => evaluateNode(a, read, seen)));
      if (nums.length === 0) throw new EvalError('AVERAGE of empty range', '#DIV/0!');
      return nums.reduce((a, b) => a + b, 0) / nums.length;
    }
    case 'MIN':
      return Math.min(...numericOnly(argNodes.map((a) => evaluateNode(a, read, seen))));
    case 'MAX':
      return Math.max(...numericOnly(argNodes.map((a) => evaluateNode(a, read, seen))));
    case 'COUNT':
      return numericOnly(argNodes.map((a) => evaluateNode(a, read, seen))).length;
    case 'COUNTA':
      return flatten(argNodes.map((a) => evaluateNode(a, read, seen))).filter(
        (v) => v !== '' && v != null,
      ).length;
    case 'SUMPRODUCT': {
      // Multiplies corresponding elements across every argument and totals
      // them: the standard way to total hours x rate per row in a fee table.
      const arrays = argNodes.map((a) => flatten([evaluateNode(a, read, seen)]));
      const [first, ...rest] = arrays;
      if (!first) return 0;
      for (const a of rest) {
        if (a.length !== first.length) {
          throw new EvalError('SUMPRODUCT: arguments must be the same size', '#VALUE!');
        }
      }
      let total = 0;
      for (let i = 0; i < first.length; i++) {
        let product = numericOrZero(first[i]);
        for (const a of rest) product *= numericOrZero(a[i]);
        total += product;
      }
      return total;
    }
    case 'ABS':
      return Math.abs(toNumber(lazy(0)[0]));
    case 'ROUND': {
      const [v, d] = lazy(0, 1);
      const factor = 10 ** toNumber(d ?? 0);
      return Math.round(toNumber(v) * factor) / factor;
    }
    case 'ROUNDUP': {
      const [v, d] = lazy(0, 1);
      const factor = 10 ** toNumber(d ?? 0);
      return Math.ceil(toNumber(v) * factor) / factor;
    }
    case 'ROUNDDOWN': {
      const [v, d] = lazy(0, 1);
      const factor = 10 ** toNumber(d ?? 0);
      return Math.floor(toNumber(v) * factor) / factor;
    }
    case 'FLOOR': {
      const [v, s] = lazy(0, 1);
      return Math.floor(toNumber(v) / toNumber(s)) * toNumber(s);
    }
    case 'CEILING': {
      const [v, s] = lazy(0, 1);
      return Math.ceil(toNumber(v) / toNumber(s)) * toNumber(s);
    }
    case 'POWER':
      return toNumber(lazy(0)[0]) ** toNumber(lazy(1)[0]);
    case 'SQRT':
      return Math.sqrt(toNumber(lazy(0)[0]));
    case 'MOD':
      return toNumber(lazy(0)[0]) % toNumber(lazy(1)[0]);
    case 'INT':
      return Math.floor(toNumber(lazy(0)[0]));
    case 'IF': {
      const cond = toNumber(evaluateNode(argNodes[0], read, seen));
      if (cond !== 0) return argNodes[1] ? evaluateNode(argNodes[1], read, seen) : true;
      return argNodes[2] ? evaluateNode(argNodes[2], read, seen) : false;
    }
    case 'IFS': {
      for (let i = 0; i + 1 < argNodes.length; i += 2) {
        if (toNumber(evaluateNode(argNodes[i], read, seen)) !== 0) {
          return evaluateNode(argNodes[i + 1], read, seen);
        }
      }
      throw new EvalError('No IFS condition matched', '#N/A');
    }
    case 'AND': {
      const vals = flatten(argNodes.map((a) => evaluateNode(a, read, seen)));
      return vals.every((v) => (typeof v === 'boolean' ? v : toNumber(v) !== 0));
    }
    case 'OR': {
      const vals = flatten(argNodes.map((a) => evaluateNode(a, read, seen)));
      return vals.some((v) => (typeof v === 'boolean' ? v : toNumber(v) !== 0));
    }
    case 'NOT':
      return toNumber(evaluateNode(argNodes[0], read, seen)) === 0;
    case 'CONCAT':
      return flatten(argNodes.map((a) => evaluateNode(a, read, seen))).map(toText).join('');
    case 'LEN':
      return toText(evaluateNode(argNodes[0], read, seen)).length;
    case 'UPPER':
      return toText(evaluateNode(argNodes[0], read, seen)).toUpperCase();
    case 'LOWER':
      return toText(evaluateNode(argNodes[0], read, seen)).toLowerCase();
    case 'TRIM':
      // Matches spreadsheet TRIM: strip the ends *and* collapse internal runs
      // of whitespace, so pasted citations do not keep double spaces.
      return toText(evaluateNode(argNodes[0], read, seen))
        .replace(/\s+/g, ' ')
        .trim();
    case 'PROPER': {
      return toText(evaluateNode(argNodes[0], read, seen)).replace(
        /\w\S*/g,
        (w) => w[0].toUpperCase() + w.slice(1).toLowerCase(),
      );
    }
    case 'LEFT':
      return toText(evaluateNode(argNodes[0], read, seen)).slice(
        0,
        Math.max(0, parseInt(String(toNumber(evaluateNode(argNodes[1], read, seen))), 10) || 1),
      );
    case 'RIGHT': {
      const s = toText(evaluateNode(argNodes[0], read, seen));
      const n = parseInt(String(toNumber(evaluateNode(argNodes[1], read, seen))), 10) || 1;
      return n <= 0 ? '' : s.slice(-n);
    }
    case 'MID': {
      const s = toText(evaluateNode(argNodes[0], read, seen));
      const start = toNumber(evaluateNode(argNodes[1], read, seen));
      const len = toNumber(evaluateNode(argNodes[2], read, seen));
      return s.substr(Math.max(0, start - 1), len);
    }
    case 'FIND': {
      const needle = toText(evaluateNode(argNodes[0], read, seen));
      const hay = toText(evaluateNode(argNodes[1], read, seen));
      const idx = hay.indexOf(needle);
      if (idx < 0) throw new EvalError('FIND: not found', '#VALUE!');
      return idx + 1;
    }
    case 'SEARCH': {
      const needle = toText(evaluateNode(argNodes[0], read, seen)).toLowerCase();
      const hay = toText(evaluateNode(argNodes[1], read, seen)).toLowerCase();
      const idx = hay.indexOf(needle);
      if (idx < 0) throw new EvalError('SEARCH: not found', '#VALUE!');
      return idx + 1;
    }
    case 'SUBSTITUTE': {
      const s = toText(evaluateNode(argNodes[0], read, seen));
      const from = toText(evaluateNode(argNodes[1], read, seen));
      const to = toText(evaluateNode(argNodes[2], read, seen));
      return s.split(from).join(to);
    }
    case 'TEXTJOIN': {
      const sep = toText(evaluateNode(argNodes[0], read, seen));
      const skipEmpty = toNumber(evaluateNode(argNodes[1], read, seen)) !== 0;
      const vals = flatten(argNodes.slice(2).map((a) => evaluateNode(a, read, seen)));
      return vals
        .filter((v) => !skipEmpty || toText(v) !== '')
        .map(toText)
        .join(sep);
    }
    case 'COUNTIF': {
      const range = flatten([evaluateNode(argNodes[0], read, seen)]);
      const crit = toText(evaluateNode(argNodes[1], read, seen));
      return range.filter((v) => matchCriterion(v, crit)).length;
    }
    case 'SUMIF': {
      const range = flatten([evaluateNode(argNodes[0], read, seen)]);
      const crit = toText(evaluateNode(argNodes[1], read, seen));
      const sumRange = argNodes[2]
        ? flatten([evaluateNode(argNodes[2], read, seen)])
        : range;
      return numericOnly(
        range
          .map((v, i) => ({ v, s: sumRange[i] }))
          .filter(({ v }) => matchCriterion(v, crit))
          .map(({ s }) => s),
      ).reduce((a, b) => a + b, 0);
    }
    case 'NOW':
      return new Date().toLocaleString();
    case 'TODAY':
      return new Date().toLocaleDateString();
    default:
      throw new EvalError(`Unknown function ${name}`, '#NAME?');
  }
}

/** Supports ">=5", "<>draft", "3", and plain equality. */
function matchCriterion(value: Scalar, criterion: string): boolean {
  const m = /^(>=|<=|<>|>|<|=)(.*)$/.exec(criterion.trim());
  if (!m) return compare(value, criterion) === 0;
  const op = m[1];
  const targetRaw = m[2].trim();
  const target: Scalar = Number.isNaN(parseFloat(targetRaw)) ? targetRaw : parseFloat(targetRaw);
  const c = compare(value, target);
  switch (op) {
    case '>=':
      return c >= 0;
    case '<=':
      return c <= 0;
    case '<>':
      return c !== 0;
    case '>':
      return c > 0;
    case '<':
      return c < 0;
    default:
      return c === 0;
  }
}

// ---------------------------------------------------------------------------
// Grid computation
// ---------------------------------------------------------------------------

/** Collects every cell key referenced by a formula, expanding ranges. */
function dependencies(node: Node, out: Set<string>): void {
  switch (node.kind) {
    case 'ref':
      out.add(node.key);
      break;
    case 'range':
      for (const k of rangeKeys(node.start, node.end)) out.add(k);
      break;
    case 'bin':
      dependencies(node.left, out);
      dependencies(node.right, out);
      break;
    case 'unary':
      dependencies(node.operand, out);
      break;
    case 'call':
      for (const a of node.args) dependencies(a, out);
      break;
    default:
      break;
  }
}

export interface ComputedCell extends Cell {
  error?: string;
}

/**
 * Recomputes every formula cell in the grid, resolving dependency order so
 * results are correct regardless of the order cells were edited in.
 */
export function computeGrid(grid: Grid): Record<string, ComputedCell> {
  const formulas = new Map<string, Node>();
  const parseErrors = new Map<string, FormulaError>();
  for (const [key, cell] of Object.entries(grid)) {
    if (cell?.f?.startsWith('=')) {
      try {
        formulas.set(key, parseFormula(cell.f));
      } catch (e) {
        // A malformed formula still owns its cell; showing the error beats
        // leaving the user staring at a blank cell with no explanation.
        parseErrors.set(key, e instanceof FormulaError ? e : new FormulaError(String(e)));
      }
    }
  }

  // Topological sort over the formula-to-formula dependency graph.
  const deps = new Map<string, Set<string>>();
  for (const [key, node] of formulas) {
    const refs = new Set<string>();
    dependencies(node, refs);
    deps.set(key, refs);
  }

  const order: string[] = [];
  const state = new Map<string, 'visiting' | 'done'>();

  const visit = (key: string, stack: Set<string>) => {
    const s = state.get(key);
    if (s === 'done') return;
    if (s === 'visiting' || stack.has(key)) {
      // Cycle: mark so the evaluator reports a circular reference.
      state.set(key, 'visiting');
      order.push(key);
      state.set(key, 'done');
      return;
    }
    state.set(key, 'visiting');
    stack.add(key);
    for (const dep of deps.get(key) ?? []) {
      if (formulas.has(dep)) visit(dep, stack);
    }
    stack.delete(key);
    order.push(key);
    state.set(key, 'done');
  };

  for (const key of formulas.keys()) visit(key, new Set());

  // Values of literal cells, resolved first.
  const literal = new Map<string, Value>();
  for (const [key, cell] of Object.entries(grid)) {
    if (!cell) continue;
    literal.set(key, cell.v ?? '');
  }

  const errored = new Map<string, string>();
  const codes = new Map<string, ErrorCode>();
  for (const [key, err] of parseErrors) {
    errored.set(key, err.message);
    codes.set(key, err.code);
  }

  const read = (key: string): Value => {
    if (errored.has(key)) {
      throw new EvalError(errored.get(key)!, codes.get(key) ?? '#ERROR!');
    }
    return literal.get(key) ?? '';
  };

  const out: Record<string, ComputedCell> = {};
  for (const [key, cell] of Object.entries(grid)) {
    if (!cell) continue;
    out[key] = errored.has(key)
      ? { ...cell, v: codes.get(key)!, error: errored.get(key)! }
      : { ...cell };
  }

  for (const key of order) {
    const node = formulas.get(key)!;
    try {
      const value = evaluateNode(node, read, new Set([key]));
      const text = formatValue(value);
      literal.set(key, text);
      out[key] = { ...out[key], v: text };
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      const code = e instanceof FormulaError ? e.code : '#ERROR!';
      errored.set(key, message);
      codes.set(key, code);
      out[key] = { ...out[key], v: code, error: message };
    }
  }

  return out;
}

export function formatValue(v: Value): string {
  if (typeof v === 'number') {
    if (!Number.isFinite(v)) return Number.isNaN(v) ? '#NUM!' : '#DIV/0!';
    return String(Math.round(v * 1e10) / 1e10);
  }
  if (typeof v === 'boolean') return v ? 'TRUE' : 'FALSE';
  if (Array.isArray(v)) return v.map(formatValue).join(', ');
  return v ?? '';
}

/** True when the value looks like a number, for right-alignment in the grid. */
export function isNumericText(v: string | undefined): boolean {
  if (!v) return false;
  return v.trim() !== '' && !Number.isNaN(parseFloat(v.replace(/[$,\s]/g, '')));
}
