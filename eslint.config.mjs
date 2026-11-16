import tseslint from 'typescript-eslint';

const asyncStateRules = {
  '@typescript-eslint/switch-exhaustiveness-check': ['error', { requireDefaultForNonUnion: false, considerDefaultExhaustiveForUnions: false }],
  '@typescript-eslint/no-floating-promises': 'error',
  '@typescript-eslint/no-misused-promises': ['error', { checksVoidReturn: { attributes: false, arguments: false } }],
  '@typescript-eslint/await-thenable': 'error',
};

export default tseslint.config(
  { ignores: ['node_modules/**', '**/dist/**', '**/target/**', 'apps/tauri/src-tauri/**', 'apps/search-demo/**', 'third_party/**', 'research/**'] },
  { linterOptions: { reportUnusedDisableDirectives: 'off' } },
  {
    files: ['apps/**/*.{ts,tsx}', 'packages/**/*.ts'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { '@typescript-eslint': tseslint.plugin },
    rules: asyncStateRules,
  },
  {
    files: ['tools/ui-audit/verify-hell-equipment-seeds.mjs'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        project: './tsconfig.e2e.json',
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: { '@typescript-eslint': tseslint.plugin },
    rules: asyncStateRules,
  },
  {
    files: ['apps/desktop/tests/**'],
    rules: {
      '@typescript-eslint/no-floating-promises': 'off',
      '@typescript-eslint/no-misused-promises': 'off',
    },
  },
);
