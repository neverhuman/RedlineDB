import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";

// TypeScript-authored flat config (loaded by ESLint via jiti). Dead-variable
// hygiene is enforced by the TypeScript compiler (noUnusedLocals /
// noUnusedParameters in tsconfig.json) plus typescript-eslint's recommended
// preset, so it is not redeclared here.
export default tseslint.config(
  {
    ignores: [
      "dist",
      "node_modules",
      "coverage",
      "playwright-report",
      "test-results",
    ],
  },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
    },
  },
  {
    // Tests may use a few non-component exports / relaxed patterns.
    files: ["**/*.test.{ts,tsx}", "test/**/*.{ts,tsx}", "e2e/**/*.ts"],
    rules: {
      "react-refresh/only-export-components": "off",
    },
  },
);
