export function NumberInput(props: {
	label: string;
	value: number;
	step?: number;
	min?: number;
	max?: number;
	required?: boolean;
	onInput: (v: number) => void;
}) {
	return (
		<label class="flex flex-col gap-1">
			<span class="text-sm opacity-70">{props.label}</span>

			<input
				class="rounded border border-gray-600 bg-gray-800 px-3 py-2"
				type="number"
				value={props.value}
				step={props.step ?? 0.1}
				min={props.min}
				max={props.max}
				required={props.required}
				onInput={(e) =>
					props.onInput(Number((e.target as HTMLInputElement).value))
				}
			/>
		</label>
	);
}

export function TextInput(props: {
	label: string;
	value: string;
	type?: string;
	required?: boolean;
	onInput: (v: string) => void;
}) {
	return (
		<label class="flex flex-col gap-1">
			<span class="text-sm opacity-70">{props.label}</span>

			<input
				class="rounded border border-gray-600 bg-gray-800 px-3 py-2"
				type={props.type ?? "text"}
				value={props.value}
				required={props.required}
				onInput={(e) =>
					props.onInput((e.target as HTMLInputElement).value)
				}
			/>
		</label>
	);
}
