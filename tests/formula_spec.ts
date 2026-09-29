import { describe, expect, it } from 'bun:test';
import {
  cellKey,
  colIndex,
  colName,
  computeGrid,
  type ComputedCell,
  formatValue,
  isNumericText,
  parseFormula,
  parseKey,
  type Grid,
} from '../src/lib/formula';

/** Converts an A1-style address to the engine's internal `row,col` key. */
function key(a1: string): string {
  const m = /^([A-Za-z]+)([0-9]+)$/.exec(a1);
  if (!m) throw new Error(`bad address ${a1}`);
  return cellKey(parseInt(m[2], 10) - 1, colIndex(m[1]));
}

/** Builds a grid from `{ 'A1': 100, 'A2': 200 }` style input. */
function g(cells: Record<string, string>): Grid {
  const out: Grid = {};
  for (const [addr, v] of Object.entries(cells)) {
    out[key(addr)] = v.startsWith('=') ? { f: v, v: '' } : { v };
  }
  return out;
}

/** Evaluates a single formula against a grid and returns its computed cell. */
function evalCell(formula: string, cells: Record<string, string> = {}): ComputedCell {
  const grid = g(cells);
  grid[key('Z9')] = { f: formula, v: '' };
  return computeGrid(grid)[key('Z9')] ?? {};
}

/** Evaluates a single formula against a grid and returns its display value. */
function evalFormula(formula: string, cells: Record<string, string> = {}): string {
  return evalCell(formula, cells).v ?? '';
}

describe('column naming', () => {
  it('round-trips names and indices', () => {
    expect(colName(0)).toBe('A');
    expect(colName(25)).toBe('Z');
    expect(colName(26)).toBe('AA');
    expect(colName(27)).toBe('AB');
    expect(colName(701)).toBe('ZZ');
    expect(colName(702)).toBe('AAA');
  });

  it('parses multi-letter columns', () => {
    expect(colIndex('A')).toBe(0);
    expect(colIndex('Z')).toBe(25);
    expect(colIndex('AA')).toBe(26);
    expect(colIndex('AZ')).toBe(51);
  });
});

describe('cell keys', () => {
  it('formats and parses consistently', () => {
    expect(cellKey(3, 7)).toBe('3,7');
    expect(parseKey('3,7')).toEqual({ row: 3, col: 7 });
  });
});

describe('arithmetic', () => {
  it('evaluates the four operations and precedence', () => {
    expect(evalFormula('=1+2*3')).toBe('7');
    expect(evalFormula('=(1+2)*3')).toBe('9');
    expect(evalFormula('=10-4-3')).toBe('3');
    expect(evalFormula('=2^3^2')).toBe('512');
  });

  it('respects unary minus', () => {
    expect(evalFormula('=-5+8')).toBe('3');
    expect(evalFormula('=-2^2')).toBe('-4');
  });

  it('reports division by zero instead of producing Infinity', () => {
    expect(evalFormula('=5/0')).toBe('#DIV/0!');
  });

  it('reads values from other cells', () => {
    expect(evalFormula('=A1+A2', { A1: '10', A2: '32' })).toBe('42');
    expect(evalFormula('=A1*2', { A1: '21' })).toBe('42');
  });

  it('strips currency formatting when reading numbers', () => {
    expect(evalFormula('=A1+1', { A1: '$1,200' })).toBe('1201');
  });
});

describe('ranges and aggregation', () => {
  it('sums a column range', () => {
    expect(evalFormula('=SUM(A1:A4)', { A1: '1', A2: '2', A3: '3', A4: '4' })).toBe('10');
  });

  it('ignores blanks and text inside SUM', () => {
    expect(evalFormula('=SUM(A1:A4)', { A1: '10', A2: '', A3: 'n/a', A4: '5' })).toBe('15');
  });

  it('computes AVERAGE over only the numeric cells', () => {
    expect(evalFormula('=AVERAGE(A1:A3)', { A1: '10', A2: '', A3: '20' })).toBe('15');
  });

  it('finds min and max', () => {
    expect(evalFormula('=MIN(A1:A3)', { A1: '9', A2: '2', A3: '5' })).toBe('2');
    expect(evalFormula('=MAX(A1:A3)', { A1: '9', A2: '2', A3: '5' })).toBe('9');
  });

  it('counts numeric and non-empty cells', () => {
    expect(evalFormula('=COUNT(A1:A4)', { A1: '1', A2: 'x', A3: '3', A4: '' })).toBe('2');
    expect(evalFormula('=COUNTA(A1:A4)', { A1: '1', A2: 'x', A3: '3', A4: '' })).toBe('3');
  });

  it('multiplies across a range with SUMPRODUCT', () => {
    // A damages table: hours x rate per row, totalled.
    const rows = Array.from({ length: 3 }, (_, i) => ({ A: String(i + 1), B: '100' }));
    const cells: Record<string, string> = {};
    rows.forEach((r, i) => {
      cells[`A${i + 1}`] = r.A;
      cells[`B${i + 1}`] = r.B;
    });
    expect(evalFormula('=SUMPRODUCT(A1:A3,B1:B3)', cells)).toBe('600');
  });
});

describe('rounding', () => {
  it('rounds to a number of decimal places', () => {
    expect(evalFormula('=ROUND(3.14159, 2)')).toBe('3.14');
    expect(evalFormula('=ROUND(2.5)')).toBe('3');
    expect(evalFormula('=ROUND(1234.5, -2)')).toBe('1200');
  });

  it('supports ROUNDUP and ROUNDDOWN', () => {
    expect(evalFormula('=ROUNDUP(3.01, 1)')).toBe('3.1');
    expect(evalFormula('=ROUNDDOWN(3.99, 1)')).toBe('3.9');
  });
});

describe('logic', () => {
  it('evaluates comparisons and AND/OR/NOT', () => {
    expect(evalFormula('=1=1')).toBe('TRUE');
    expect(evalFormula('=1<>1')).toBe('FALSE');
    expect(evalFormula('=2>1')).toBe('TRUE');
    expect(evalFormula('=AND(TRUE,1=1)')).toBe('TRUE');
    expect(evalFormula('=OR(FALSE,FALSE)')).toBe('FALSE');
    expect(evalFormula('=NOT(TRUE)')).toBe('FALSE');
  });

  it('only evaluates the taken branch of IF', () => {
    // The condition is false, so the 1/A1 branch must never be evaluated --
    // an eager implementation would surface #DIV/0! here.
    expect(evalFormula('=IF(A1=1, 1/A1, 5)', { A1: '0' })).toBe('5');
    expect(evalFormula('=IF(A1=0, 1/A1, 5)', { A1: '0' })).toBe('#DIV/0!');
    expect(evalFormula('=IF(A1=3, "yes", "no")', { A1: '3' })).toBe('yes');
  });

  it('walks IFS to the first matching condition', () => {
    expect(evalFormula('=IFS(A1>10,"big",A1>5,"mid",TRUE,"small")', { A1: '7' })).toBe('mid');
    expect(evalFormula('=IFS(A1>10,"big",A1>5,"mid",TRUE,"small")', { A1: '1' })).toBe('small');
  });
});

describe('text', () => {
  it('concatenates and cleans up', () => {
    expect(evalFormula('="Case "&A1', { A1: 'CV-42' })).toBe('Case CV-42');
    expect(evalFormula('=UPPER("hearing")')).toBe('HEARING');
    expect(evalFormula('=TRIM("  spaced  out  ")')).toBe('spaced out');
    expect(evalFormula('=LEN("statute")')).toBe('7');
  });

  it('slices with LEFT, RIGHT and MID', () => {
    expect(evalFormula('=LEFT("Henderson v. Northgate", 9)')).toBe('Henderson');
    expect(evalFormula('=RIGHT("2026-CV-0042", 4)')).toBe('0042');
    expect(evalFormula('=MID("abcdef", 2, 3)')).toBe('bcd');
  });

  it('finds and replaces', () => {
    expect(evalFormula('=FIND("v.", "Henderson v. Northgate")')).toBe('11');
    expect(evalFormula('=SUBSTITUTE("a-b-c", "-", " ")')).toBe('a b c');
  });

  it('joins a list with TEXTJOIN', () => {
    expect(evalFormula('=TEXTJOIN(", ",TRUE,A1:A3)', { A1: 'Miriam', A2: 'Ken', A3: '' })).toBe(
      'Miriam, Ken',
    );
  });
});

describe('conditional aggregation', () => {
  it('counts rows matching a criterion', () => {
    const cells = { A1: 'Open', A2: 'Closed', A3: 'Open', A4: 'Pending' };
    expect(evalFormula('=COUNTIF(A1:A4,"Open")', cells)).toBe('2');
  });

  it('sums only rows matching a criterion', () => {
    const cells = { A1: 'Open', A2: 'Closed', A3: 'Open', B1: '100', B2: '200', B3: '300' };
    expect(evalFormula('=SUMIF(A1:A3,"Open",B1:B3)', cells)).toBe('400');
  });

  it('supports comparison criteria', () => {
    const cells = { A1: '5', A2: '15', A3: '25' };
    expect(evalFormula('=COUNTIF(A1:A3,">10")', cells)).toBe('2');
    expect(evalFormula('=COUNTIF(A1:A3,"<>15")', cells)).toBe('2');
  });
});

describe('errors and cycles', () => {
  it('reports a circular reference rather than hanging', () => {
    const cell = evalCell('=A1+1', { A1: '=A1+1' });
    expect(cell.v).toBe('#CIRCULAR!');
    expect(cell.error).toContain('Circular');
  });

  it('reports an unknown function', () => {
    const cell = evalCell('=NOPE(1)');
    expect(cell.v).toBe('#NAME?');
    expect(cell.error).toContain('Unknown function');
  });

  it('reports a parse error', () => {
    const cell = evalCell('=SUM(');
    expect(cell.v).toBe('#ERROR!');
    expect(cell.error).toBeTruthy();
  });

  it('propagates an error through a dependent cell', () => {
    const grid = g({ A1: '=1/0', A2: '=A1+1' });
    const out = computeGrid(grid);
    expect(out[key('A1')].error).toBeTruthy();
    expect(out[key('A2')].error).toBeTruthy();
  });
});

describe('dependency ordering', () => {
  it('resolves a forward reference regardless of edit order', () => {
    // A2 references A1, but A1 is listed first in this literal only by chance.
    const grid = g({ A1: '10', A2: '=A1*2', A3: '=A2+5' });
    const out = computeGrid(grid);
    expect(out[key('A2')].v).toBe('20');
    expect(out[key('A3')].v).toBe('25');
  });

  it('recomputes when an upstream value changes', () => {
    const first = computeGrid(g({ A1: '5', A2: '=A1*10' }));
    expect(first[key('A2')].v).toBe('50');
    const second = computeGrid(g({ A1: '6', A2: '=A1*10' }));
    expect(second[key('A2')].v).toBe('60');
  });
});

describe('formatters', () => {
  it('detects numeric text for alignment', () => {
    expect(isNumericText('1234')).toBe(true);
    expect(isNumericText('$1,200.50')).toBe(true);
    expect(isNumericText('Open')).toBe(false);
    expect(isNumericText('')).toBe(false);
  });

  it('formats values for display', () => {
    expect(formatValue(1234.5)).toBe('1234.5');
    expect(formatValue(true)).toBe('TRUE');
    expect(formatValue([1, 2])).toBe('1, 2');
  });
});

describe('parser', () => {
  it('handles absolute references', () => {
    expect(() => parseFormula('=$A$1+B$2')).not.toThrow();
  });

  it('rejects malformed input', () => {
    expect(() => parseFormula('=1+')).toThrow();
  });
});
