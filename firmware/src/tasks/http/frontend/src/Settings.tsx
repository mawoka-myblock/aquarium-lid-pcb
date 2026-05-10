import { useEffect, useState } from "preact/hooks";
import { Api, type Config, type MQTTConfig } from "./api";
import { NumberInput, TextInput } from "./components/Inputs";
import { Card, FoldoutCard } from "./components/Card";

export function Settings() {
	const [config, setConfig] = useState<Config>();
	const [mqtt, setMqtt] = useState<MQTTConfig>();

	useEffect(() => {
		(async () => {
			setConfig(await Api.get_config());
			setMqtt({
				client_id: "",
				host: "",
				password: "",
				username: "",
				port: 1883,
			});
		})();
	}, []);

	const save_mqtt = async (e) => {
		e.preventDefault();
		await Api.post_mqtt(mqtt);
		window.location.reload();
	};

	const save_settings = async (e) => {
		e.preventDefault();
		await Api.post_config(config);
		window.location.reload();
	};

	if (!config) return <div>Loading...</div>;

	return (
		<Card title="Config">
			<form onSubmit={save_settings} class="group">
				<FoldoutCard title="Fan Config">
					<NumberInput
						label="Fan on"
						value={Math.round(config.fan_on_threshold * 10) / 10}
						required={true}
						onInput={(v) =>
							setConfig({ ...config, fan_on_threshold: v })
						}
					/>
					<NumberInput
						label="Fan off"
						value={Math.round(config.fan_off_threshold * 10) / 10}
						required={true}
						onInput={(v) =>
							setConfig({ ...config, fan_off_threshold: v })
						}
					/>
				</FoldoutCard>
				<FoldoutCard title="Alarm Config">
					<NumberInput
						label="Max. safe temperature"
						value={Math.round(config.max_safe_temp * 10) / 10}
						required={true}
						onInput={(v) =>
							setConfig({ ...config, max_safe_temp: v })
						}
					/>
					<NumberInput
						label="Min. safe temperature"
						value={Math.round(config.min_safe_temp * 10) / 10}
						required={true}
						onInput={(v) =>
							setConfig({ ...config, min_safe_temp: v })
						}
					/>
					<NumberInput
						label="Alarm hysterisis"
						step={0.1}
						min={0}
						max={2}
						required={true}
						value={Math.round(config.alarm_hysteresis * 10) / 10}
						onInput={(v) =>
							setConfig({ ...config, alarm_hysteresis: v })
						}
					/>
				</FoldoutCard>
				<FoldoutCard title="LED Config">
					<NumberInput
						label="LED brightness (%)"
						step={1}
						min={0}
						max={100}
						required={true}
						value={Math.round((config.led_brightness / 255) * 100)}
						onInput={(v) =>
							setConfig({
								...config,
								led_brightness: Math.round((v / 100) * 255),
							})
						}
					/>
				</FoldoutCard>
				<button class="mb-4 p-2 group-invalid:opacity-60" type="submit">
					Save general settings
				</button>
			</form>
			<FoldoutCard title="MQTT Config">
				<form class="group" onSubmit={save_mqtt}>
					<TextInput
						label="MQTT Host"
						type="text"
						value={mqtt.host}
						required={true}
						onInput={(v) => setMqtt({ ...mqtt, host: v })}
					/>
					<NumberInput
						label="MQTT Port"
						step={1}
						min={0}
						max={999999}
						value={mqtt.port}
						required={true}
						onInput={(v) => setMqtt({ ...mqtt, port: v })}
					/>
					<TextInput
						label="MQTT Username"
						type="text"
						required={true}
						value={mqtt.username}
						onInput={(v) => setMqtt({ ...mqtt, username: v })}
					/>
					<TextInput
						label="MQTT Password"
						type="password"
						required={true}
						value={mqtt.password}
						onInput={(v) => setMqtt({ ...mqtt, password: v })}
					/>
					<button
						class="mt-4 p-2 group-invalid:opacity-60"
						type="submit"
					>
						Save MQTT settings!
					</button>
				</form>
			</FoldoutCard>
		</Card>
	);
}
