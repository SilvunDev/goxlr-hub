import type { Messages } from './en';

export const fr: Messages = {
  disclaimer: 'Non officiel. Non affilié à TC-Helicon.',
  nav: {
    label: 'Sections',
    mixer: 'Table',
    mic: 'Micro',
    channels: 'Pistes audio',
    routing: 'Routage',
    controls: 'Touches',
    lighting: 'Éclairage',
    plugins: 'Plugins',
    settings: 'Réglages',
  },
  profile: {
    label: 'Profil',
    none: 'Aucun profil pour l’instant',
  },
  comingSoon: {
    tag: 'À venir',
    body: 'Cette section n’est pas encore construite. Elle arrivera dans une prochaine version.',
  },
  connection: {
    demo: {
      tag: 'Mode démonstration',
      body: 'Aucune GoXLR branchée. Vous voyez un appareil virtuel.',
    },
    busy: {
      tag: 'GoXLR occupée',
      body: 'GoXLR Hub a besoin de la GoXLR pour lui seul. Quittez {program} : la connexion se fait ensuite toute seule. En attendant, vous voyez un appareil virtuel.',
      otherProgram: 'l’autre logiciel GoXLR',
    },
    unsupported: {
      tag: 'GoXLR Mini',
      body: 'La GoXLR Mini n’est pas prise en charge : GoXLR Hub fonctionne avec la GoXLR complète. Vous voyez un appareil virtuel.',
    },
    unreachable: {
      tag: 'GoXLR injoignable',
      body: 'Une GoXLR est branchée mais ne répond pas. Nouvel essai en cours… En attendant, vous voyez un appareil virtuel.',
    },
  },
  mixer: {
    connecting: 'Connexion…',
    readOnly: 'Aperçu : vous pourrez manipuler ces commandes dans une prochaine version.',
    fader: 'Fader',
    muted: 'Coupé',
    live: 'Ouvert',
    micLevel: 'Niveau du micro',
  },
  device: {
    title: 'Appareil',
    virtual: 'GoXLR virtuelle',
    hardware: 'GoXLR',
    firmware: 'Firmware',
    serial: 'Numéro de série',
  },
  channels: {
    mic: 'Micro',
    lineIn: 'Entrée ligne',
    console: 'Console',
    system: 'Système',
    game: 'Jeu',
    chat: 'Chat',
    sample: 'Sampler',
    music: 'Musique',
    headphones: 'Casque',
    micMonitor: 'Retour micro',
    lineOut: 'Sortie ligne',
  },
  settings: {
    language: 'Langue',
  },
};
