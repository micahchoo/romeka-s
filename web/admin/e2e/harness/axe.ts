/**
 * The axe-core accessibility check the specs run once per page.
 *
 * Every page the suite visits is scanned against the WCAG 2.0 and 2.1 A and AA
 * rules plus axe's best-practice rules, so the page is judged whole. The budget
 * is zero serious and zero critical violations. The moderate and minor counts
 * are printed on every run, whether the budget passes or fails, so a regression
 * is visible in the log before it turns into a serious one.
 *
 * `expectAxeBudget` is the assertion. `AXE_TAGS` is the rule set, exported so
 * the README and a reviewer can name it.
 */
import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { expect } from '@playwright/test';

/** The rule tags a page is judged against. */
export const AXE_TAGS: readonly string[] = [
  'wcag2a',
  'wcag2aa',
  'wcag21a',
  'wcag21aa',
  'best-practice',
];

type AxeResults = Awaited<ReturnType<AxeBuilder['analyze']>>;
type AxeViolation = AxeResults['violations'][number];

type Impact = 'minor' | 'moderate' | 'serious' | 'critical';

const impactOf = (violation: AxeViolation): Impact | 'unknown' => {
  switch (violation.impact) {
    case 'minor':
    case 'moderate':
    case 'serious':
    case 'critical':
      return violation.impact;
    default:
      return 'unknown';
  }
};

/** The selectors of every node a rule flagged, so a failure names the element. */
const targetsOf = (violation: AxeViolation): string =>
  violation.nodes.map((node) => node.target.join(' ')).join(', ');

const describeViolations = (violations: readonly AxeViolation[]): string =>
  violations
    .map(
      (violation) =>
        `    ${impactOf(violation)} ${violation.id} x${String(violation.nodes.length)}: ${violation.help} [${targetsOf(violation)}]`,
    )
    .join('\n');

/**
 * Run axe over the loaded page, print its counts and violations, and assert the
 * serious and critical budget. `label` names the page in the log and in a
 * failure message.
 */
export const expectAxeBudget = async (page: Page, label: string): Promise<AxeResults> => {
  const results = await new AxeBuilder({ page }).withTags([...AXE_TAGS]).analyze();
  const violations = results.violations.filter((violation) => violation.nodes.length > 0);
  const count = (impact: Impact): number =>
    violations.filter((violation) => impactOf(violation) === impact).length;
  const faults = violations.filter(
    (violation) => impactOf(violation) === 'serious' || impactOf(violation) === 'critical',
  );

  console.log(
    `[a11y] ${label}: serious=${String(count('serious'))} critical=${String(count('critical'))} moderate=${String(count('moderate'))} minor=${String(count('minor'))}`,
  );
  if (violations.length > 0) {
    console.log(describeViolations(violations));
  }

  expect(
    faults,
    `${label}: axe found serious or critical violations\n${describeViolations(faults)}`,
  ).toEqual([]);
  return results;
};
