/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        surface: {
          light: '#FFFFFF',
          dark: '#1E293B',
        },
        background: {
          light: '#FAFAFA',
          dark: '#0F172A',
        },
        accent: {
          DEFAULT: '#0D9488',
          hover: '#0F766E',
          dark: '#2DD4BF',
          'dark-hover': '#5EEAD4',
        },
      },
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', '"Segoe UI"', 'sans-serif'],
        // The bundled variable font registers under this exact name; plain
        // "JetBrains Mono" is kept for anyone who has it installed locally.
        mono: ['"JetBrains Mono Variable"', '"JetBrains Mono"', '"SF Mono"', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
}
