import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  // `tests/lint/` holds files whose only purpose is to be refused by a rule
  // below; `tests/annotations-wire.test.ts` runs ESLint over them by name.
  // `.evidence/` holds the throwaway Playwright scripts a unit keeps its screenshots with.
  { ignores: ['dist/', 'tests/lint/', '.evidence/'] },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.strictTypeChecked],
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/consistent-type-assertions': ['error', { assertionStyle: 'never' }],
    },
  },
  {
    files: ['perf/**/*.mjs', 'scripts/**/*.mjs'],
    extends: [js.configs.recommended],
    languageOptions: {
      globals: globals.node,
    },
  },
  {
    files: ['tests/**/*.{ts,tsx}', 'e2e/**/*.ts', 'vite.config.ts'],
    languageOptions: {
      globals: globals.node,
    },
    rules: {
      // Playwright's fixture signature destructures the dependencies it needs, and
      // the `baseURL` override needs none of them.
      'no-empty-pattern': 'off',
    },
  },
  {
    // The notes folder is a lazy chunk with one door: the `lazy()` import in
    // `pages/media/detail.tsx`. Its own tests are named `annotations-*`, so a
    // test that needs the wire has to say so in its file name.
    files: ['src/**/*.{ts,tsx}', 'tests/**/*.{ts,tsx}'],
    ignores: [
      'src/pages/media/annotations/**',
      'src/pages/media/detail.tsx',
      'tests/annotations-*.test.{ts,tsx}',
    ],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['**/pages/media/annotations', '**/pages/media/annotations/*'],
              message: 'Open the notes panel only through its lazy import on the media page.',
            },
          ],
        },
      ],
    },
  },
  {
    // A field has one name on every admin screen: `plainName` in `src/app/plain-terms.ts` (the item
    // type's own name, then the curated plain name, then the label). Only Standards shows the
    // vocabularies' own labels, and the class picker names classes; nothing else reads a term's label.
    files: ['src/**/*.{ts,tsx}', 'tests/lint/field-name-*.ts'],
    ignores: [
      'src/app/plain-terms.ts',
      'src/app/class-choices.ts',
      'src/pages/standards/**',
      'src/pages/vocabularies/**',
      'src/pages/properties/**',
      'src/pages/resource-classes/**',
    ],
    rules: {
      'no-restricted-syntax': [
        'error',
        {
          selector: "CallExpression[callee.property.name='label'][arguments.0.property.name='term']",
          message: 'Name a field with plainName (src/app/plain-terms.ts), so every screen calls it the same.',
        },
        {
          selector: "CallExpression[callee.property.name='label'][arguments.0.name='term']",
          message: 'Name a field with plainName (src/app/plain-terms.ts), so every screen calls it the same.',
        },
      ],
    },
  },
);
