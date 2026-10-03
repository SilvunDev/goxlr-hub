export const en = {
  disclaimer: 'Unofficial. Not affiliated with TC-Helicon.',
  nav: {
    label: 'Sections',
    mixer: 'Mixer',
    mic: 'Microphone',
    channels: 'Channels',
    routing: 'Routing',
    controls: 'Controls',
    lighting: 'Lighting',
    plugins: 'Plugins',
    settings: 'Settings',
  },
  profile: {
    label: 'Profile',
    none: 'No profile yet',
  },
  comingSoon: {
    tag: 'Coming soon',
    body: 'This section is not built yet. It will arrive in a later version.',
  },
  // Shown above the virtual device, to say why the real one is not there.
  connection: {
    demo: {
      tag: 'Demo mode',
      body: 'No GoXLR connected. You are looking at a virtual device.',
    },
    busy: {
      tag: 'GoXLR in use',
      body: 'GoXLR Hub needs the GoXLR for itself. Quit {program}: the connection is then automatic. Meanwhile, you are looking at a virtual device.',
      otherProgram: 'the other GoXLR program',
    },
    unsupported: {
      tag: 'GoXLR Mini',
      body: 'The GoXLR Mini is not supported: GoXLR Hub works with the full-size GoXLR. You are looking at a virtual device.',
    },
    unreachable: {
      tag: 'GoXLR unreachable',
      body: 'A GoXLR is plugged in but does not answer. Trying again… Meanwhile, you are looking at a virtual device.',
    },
  },
  mixer: {
    connecting: 'Connecting…',
    fader: 'Fader',
    source: 'Channel of fader {fader}',
    mute: 'Mute {channel}',
    mic: 'Microphone',
    micHint: 'Like the microphone button of the GoXLR: nobody hears you, whatever the mixer says.',
    micOff: 'Turn the microphone off',
    muted: 'Muted',
    live: 'Live',
    micLevel: 'Microphone level',
  },
  channelList: {
    hint: 'The volume of every channel, including those that are on no fader.',
    unknown: 'Unknown',
    unknownHint:
      'The GoXLR cannot tell the volume of a channel that is on no fader. It shows here once you set it.',
  },
  routing: {
    hint: 'Tick where each source is sent. The GoXLR follows at once.',
    source: 'Source',
    route: '{input} to {output}',
    loopHint: 'A dash marks a route that would send a sound back to where it comes from.',
    inputs: {
      mic: 'Mic',
      chat: 'Chat',
      music: 'Music',
      game: 'Game',
      console: 'Console',
      lineIn: 'Line In',
      system: 'System',
      samples: 'Samples',
    },
    outputs: {
      headphones: 'Headphones',
      broadcastMix: 'Broadcast Mix',
      lineOut: 'Line Out',
      chatMic: 'Chat Mic',
      sampler: 'Sampler',
    },
    outputHints: {
      headphones: 'What you hear.',
      broadcastMix: 'What your stream or your recording gets.',
      lineOut: 'The line output at the back of the GoXLR.',
      chatMic: 'What the people in your voice chat hear.',
      sampler: 'What the sampler records.',
    },
  },
  device: {
    title: 'Device',
    virtual: 'Virtual GoXLR',
    hardware: 'GoXLR',
    firmware: 'Firmware',
    serial: 'Serial number',
  },
  channels: {
    mic: 'Mic',
    lineIn: 'Line In',
    console: 'Console',
    system: 'System',
    game: 'Game',
    chat: 'Chat',
    sample: 'Sampler',
    music: 'Music',
    headphones: 'Headphones',
    micMonitor: 'Mic Monitor',
    lineOut: 'Line Out',
  },
  settings: {
    language: 'Language',
  },
};

export type Messages = typeof en;
