import { defineConfig, presetUno, presetIcons, presetAttributify } from 'unocss'

export default defineConfig({
	presets: [
		presetAttributify({
		}),
		presetUno(),
		presetIcons({
			cdn: 'https://esm.sh/'
		}),
	],
  shortcuts: [
    {
			"btn-base": "rounded-0.5rem b-0.2rem pointer",
			"btn-white": "btn-base text-dark100 bg-light500 hover-bg-light300 active-bg-light900",
			"btn-emerald": "btn-base text-light100 bg-emerald600 hover-bg-emerald500 active-bg-emerald700"
		}
  ]
})
