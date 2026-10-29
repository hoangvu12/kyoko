local theme = require("../theme")
local state = require("../state")
local actions = require("../actions")
local kit = require("./kit")
local keybind = require("./keybind")

local SENSITIVITY_FLOOR_DB = -100
local VOLUME_MAX = 200
local DEFAULT_DEVICE = "Default"
local RELEASE_DELAY_MAX_MS = 2000
local PREVIEW_RATIO = 9 / 16

local function push_to_talk(): boolean
	return kit.value("voice.push_to_talk") == true
end

local function manual_sensitivity(): boolean
	return not push_to_talk() and kit.value("voice.auto_sensitivity") == false
end

local function sensitivity_db(): number
	local found = kit.value("voice.sensitivity_db")
	return if type(found) == "number" then found else -50
end

local function fraction_of(db: number): number
	return math.clamp((db - SENSITIVITY_FLOOR_DB) / -SENSITIVITY_FLOOR_DB, 0, 1)
end

local function device_names(list: { state.AudioDevice }): { string }
	local names = { DEFAULT_DEVICE }
	for _, device in list do
		table.insert(names, device.name)
	end
	return names
end

local function device_select(id: string, key: string, list: () -> { state.AudioDevice })
	return ui.presence(function()
		local ids = {}
		for _, device in list() do
			table.insert(ids, device.id .. "=" .. device.name)
		end
		return table.concat(ids, "\n")
	end, function()
		local devices = list()
		return kit.select {
			id = id,
			labels = device_names(devices),
			current = function()
				local chosen = kit.value(key)
				for index, device in devices do
					if device.id == chosen then
						return index + 1
					end
				end
				return 1
			end,
			pick = function(index: number)
				local device = devices[index - 1]
				kit.set(key, if device then device.id else "")
			end,
		}
	end)
end

local function inputs(): { state.AudioDevice }
	return state.voice.devices:get().inputs
end

local function outputs(): { state.AudioDevice }
	return state.voice.devices:get().outputs
end

local function pair(left: any, right: any)
	return ui.row {
		w_full = true,
		gap = theme.size.space_16,
		ui.column { grow = 1, basis = 0, min_w = 0, gap = theme.size.space_8, left },
		ui.column { grow = 1, basis = 0, min_w = 0, gap = theme.size.space_8, right },
	}
end

local function cameras(): { state.AudioDevice }
	return state.voice.cameras:get()
end

local function previewing(): boolean
	return state.voice.camera_preview:get()
end

local function video_available(): boolean
	return state.voice.video_available:get()
end

local function camera_preview()
	local box = ui.ref()
	return ui.div {
		ref = box,
		id = "voice_camera_preview",
		relative = true,
		w_full = true,
		h = function()
			return (box.width or 0) * PREVIEW_RATIO
		end,
		rounded = theme.size.radius_md,
		overflow = "hidden",
		bg = theme.color.bg_2,
		justify = "center",
		items = "center",
		ui.show(
			previewing,
			ui.image {
				id = "voice_camera_preview_image",
				src = "host:video/preview",
				absolute = true,
				left = 0,
				top = 0,
				w_full = true,
				h_full = true,
				fit = "cover",
			}
		),
		ui.show(function()
			return not previewing()
		end, ui.presence(function()
			return if video_available() then "ready" else "unavailable"
		end, function()
			if not video_available() then
				return kit.text(
					"Video unavailable",
					theme.size.font_size_md,
					theme.size.line_height_label,
					theme.color.text_4,
					{ id = "voice_camera_unavailable" }
				)
			end
			return kit.button {
				id = "voice_camera_test",
				text = "Test Video",
				tone = "primary",
				height = theme.size.button_height_md,
				on_click = function()
					actions.set_camera_preview(true)
				end,
			}
		end)),
	}
end

local function camera()
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.caption("Camera"),
		device_select("voice_camera_device", "voice.camera_device", cameras),
		ui.column {
			w_full = true,
			pt = theme.size.space_8,
			gap = theme.size.space_12,
			camera_preview(),
			ui.show(
				previewing,
				ui.row {
					w_full = true,
					justify = "end",
					kit.button {
						id = "voice_camera_stop",
						text = "Stop Video",
						tone = "secondary",
						height = theme.size.button_height_md,
						on_click = function()
							actions.set_camera_preview(false)
						end,
					},
				}
			),
		},
	}
end

local function devices()
	return pair(
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			kit.caption("Input Device"),
			device_select("voice_input_device", "voice.input_device", inputs),
		},
		ui.column {
			w_full = true,
			gap = theme.size.space_8,
			kit.caption("Output Device"),
			device_select("voice_output_device", "voice.output_device", outputs),
		}
	)
end

local function volume(title: string, key: string)
	return ui.column {
		w_full = true,
		gap = theme.size.space_8,
		kit.caption(title),
		kit.slider { key = key, min = 0, max = VOLUME_MAX, step = 1, scale = 1 },
	}
end

local function volumes()
	return pair(volume("Input Volume", "voice.input_volume"), volume("Output Volume", "voice.output_volume"))
end

local function meter(id: string, level: () -> number, threshold: (() -> number)?)
	local track = ui.ref()
	return ui.div {
		ref = track,
		id = id,
		relative = true,
		grow = 1,
		min_w = 0,
		h = theme.size.slider_track_height,
		rounded = theme.size.slider_track_height,
		bg = theme.color.bg_2,
		overflow = "hidden",
		ui.div {
			id = id .. "_level",
			absolute = true,
			left = 0,
			top = 0,
			bottom = 0,
			w = function()
				return (track.width or 0) * level()
			end,
			rounded = theme.size.slider_track_height,
			bg = function()
				if threshold and level() < threshold() then
					return theme.color.yellow_2
				end
				return theme.color.green_2
			end,
		},
	}
end

local function mic_level(): number
	return state.voice.mic_test:get().level
end

local function testing(): boolean
	return state.voice.mic_test:get().running
end

local function mic_test()
	return ui.column {
		w_full = true,
		gap = theme.size.space_12,
		kit.label("Mic Test", "Start a test and say something. Kyoko plays your voice back to you."),
		ui.row {
			w_full = true,
			items = "center",
			gap = theme.size.space_16,
			ui.presence(function()
				return if testing() then "stop" else "start"
			end, function()
				local running = testing()
				return kit.button {
					id = "voice_mic_test",
					text = if running then "Stop Testing" else "Let's Check",
					tone = if running then "secondary" else "primary",
					height = theme.size.button_height_md,
					on_click = function()
						actions.set_mic_test(not testing())
					end,
				}
			end),
			meter("voice_mic_meter", mic_level),
		},
	}
end

local sensitivity = ui.component(function()
	local track = ui.ref()
	local dragging = ui.use_state(false)
	local held = ui.use_state(nil :: number?)
	local function current(): number
		local value = held:get()
		return if value ~= nil then value else sensitivity_db()
	end
	local function pick(x: number)
		local width = track.width or 0
		if width <= 0 then
			return
		end
		held:set(math.floor(SENSITIVITY_FLOOR_DB * (1 - math.clamp(x / width, 0, 1)) + 0.5))
	end
	local function commit()
		local value = held:get()
		dragging:set(false)
		if value ~= nil and value ~= sensitivity_db() then
			kit.set("voice.sensitivity_db", value)
		end
		held:set(nil)
	end
	local grabber = theme.number(theme.size.slider_grabber_size)
	local bar = theme.number(theme.size.slider_track_height)
	return ui.row {
		w_full = true,
		items = "center",
		gap = theme.size.space_16,
		ui.div {
			ref = track,
			id = "voice_sensitivity",
			label = "Input Sensitivity",
			relative = true,
			grow = 1,
			h = theme.size.slider_grabber_size,
			cursor = "pointer",
			on_press = function(event: any)
				dragging:set(true)
				pick(event.local_x)
			end,
			on_pointer_move = function(event: any)
				if dragging:get() then
					pick(event.local_x)
				end
			end,
			on_release = commit,
			on_hover = function(inside: boolean)
				if not inside and dragging:get() then
					commit()
				end
			end,
			ui.row {
				absolute = true,
				left = 0,
				right = 0,
				top = function()
					return (grabber() - bar()) / 2
				end,
				meter("voice_sensitivity_meter", mic_level, function()
					return fraction_of(current())
				end),
			},
			ui.div {
				absolute = true,
				top = 0,
				left = function()
					return (track.width or 0) * fraction_of(current()) - grabber() / 2
				end,
				w = theme.size.slider_grabber_size,
				h = theme.size.slider_grabber_size,
				rounded = theme.size.slider_grabber_size,
				bg = theme.color.text_1,
			},
		},
		kit.text(function()
			return string.format("%d dB", current())
		end, theme.size.font_size_sm, theme.size.line_height_label, theme.color.text_4, {
			w = theme.size.space_56,
			shrink = 0,
		}),
	}
end)

local function sensitivity_row()
	return ui.column {
		w_full = true,
		gap = theme.size.space_12,
		kit.label(
			"Input Sensitivity",
			"Kyoko sends your voice once it is louder than the marker. Run the mic test to see your level against it."
		),
		sensitivity(),
	}
end

return {
	headings = {
		{
			key = "voice",
			title = "Voice",
			rows = {
				{
					kind = "block",
					title = "Input Device",
					keywords = "microphone mic headset output device speakers headphones",
					build = devices,
				},
				{
					kind = "block",
					title = "Input Volume",
					keywords = "output volume loudness gain louder quieter",
					build = volumes,
				},
				{
					kind = "block",
					title = "Mic Test",
					keywords = "let's check level meter microphone test hear yourself",
					build = mic_test,
				},
				{
					kind = "radios",
					key = "voice.push_to_talk",
					id = "voice_input_mode",
					title = "Input Mode",
					keywords = "voice activity push to talk ptt",
					options = {
						{ label = "Voice Activity", value = false },
						{ label = "Push to Talk", value = true },
					},
				},
				{
					kind = "block",
					title = "Push to Talk Shortcut",
					keywords = "keybind hotkey key ptt",
					visible = push_to_talk,
					build = function()
						return ui.column {
							w_full = true,
							gap = theme.size.space_8,
							keybind {
								key = "keybinds.push_to_talk",
								label = "Push to Talk",
								id = "voice_push_to_talk_record",
								mouse = true,
							},
							kit.hint(
								"Hold the shortcut to talk. It works while Kyoko is in the background, and the key still reaches the app you are using."
							),
							kit.hint(
								"A game run as administrator needs Kyoko run as administrator too, or the shortcut won't work while the game is focused."
							),
						}
					end,
				},
				{
					kind = "slider",
					key = "voice.push_to_talk_release_delay",
					id = "voice_release_delay",
					title = "Push to Talk Release Delay",
					note = "How long your mic stays open after you let go of the shortcut.",
					keywords = "ptt release delay hold",
					visible = push_to_talk,
					min = 0,
					max = RELEASE_DELAY_MAX_MS,
					step = 1,
					scale = 1,
					unit = " ms",
				},
				{
					key = "voice.auto_sensitivity",
					id = "voice_auto_sensitivity",
					title = "Automatically determine input sensitivity",
					keywords = "input sensitivity threshold gate automatic",
					visible = function()
						return not push_to_talk()
					end,
				},
				{
					kind = "block",
					title = "Input Sensitivity",
					keywords = "threshold gate decibels db",
					visible = manual_sensitivity,
					build = sensitivity_row,
				},
			},
		},
		{
			key = "video",
			title = "Video Settings",
			rows = {
				{
					kind = "block",
					title = "Camera",
					keywords = "camera webcam video preview test video",
					build = camera,
				},
			},
		},
		{
			key = "voice-processing",
			title = "Voice Processing",
			rows = {
				{
					kind = "radios",
					key = "voice.noise_suppression",
					id = "voice_noise_suppression",
					title = "Noise Suppression",
					note = "Removes background noise from your microphone before anyone hears it.",
					keywords = "noise background krisp deepfilternet rnnoise filter fan keyboard",
					options = {
						{
							label = "High Quality",
							value = "high_quality",
							note = "Filters out fans, typing, traffic and other noise around you. Uses more CPU.",
						},
						{
							label = "Low CPU",
							value = "low_cpu",
							note = "Lighter filtering for steady noise, like fans, for older computers.",
						},
						{ label = "Off", value = "off", note = "Sends your microphone as it is." },
					},
				},
				{
					key = "voice.echo_cancellation",
					id = "voice_echo_cancellation",
					title = "Echo Cancellation",
					note = "Stops what plays from your speakers from reaching your microphone, so others don't hear themselves.",
					keywords = "echo cancellation feedback speakers aec",
				},
				{
					key = "voice.auto_gain_control",
					id = "voice_auto_gain_control",
					title = "Automatic Gain Control",
					note = "Evens out your microphone volume so you're never too quiet or too loud.",
					keywords = "automatic gain control agc volume level loudness",
				},
			},
		},
	}
}
