package com.monolito.correlula;

/** Frases e memes do caminho. Parodia sem fins lucrativos. */
final class Memes {
    private Memes() {}

    /** Outdoors na beira da pista ('\n' quebra linha). */
    static final String[] BILLBOARDS = {
            "FAZ O L!",
            "O AMOR\nVENCEU",
            "PICANHA\nPRA TODOS",
            "O BRASIL\nVOLTOU",
            "COMPANHEIRADA!",
            "NUNCA ANTES NA\nHISTÓRIA DESTE PAÍS",
            "NÃO VOU PAGAR\nO PATO",
            "ESTOCAR\nVENTO",
            "SAUDANDO\nA MANDIOCA",
            "ATINGIU A META?\nDOBRA A META!",
            "TAXA DAS\nBLUSINHAS",
            "SEXTOU,\nCOMPANHEIRO!",
            "É POBRE\nANDANDO DE AVIÃO",
            "PICANHA\nR$ 9,99",
            "TREM-BALA:\nEM BREVE (DESDE 2010)",
            "VAI\nCORINTHIANS!",
            "A ESPERANÇA\nVENCEU O MEDO",
            "CALMA,\nCALABRESO",
            "É VERDADE\nESSE BILETE",
            "LULA LÁ",
            "OLHA O PATO!",
            "BOLSA\nPICANHA",
            "MENOS JUROS,\nMAIS PICANHA",
            "FAZ O PIX\nDA PICANHA",
            "TÁ PAGO!",
            "CHURRASCO\nNA LAJE",
            "CORRE QUE\nO PATO VEM",
            "SEM PICANHA\nNÃO DÁ",
    };

    /** Faixas das barreiras altas (abaixe para passar). */
    static final String[] BANNERS = {
            "FAKE NEWS", "JUROS ALTOS", "INFLAÇÃO", "TAXA DAS\nBLUSINHAS", "BOLETOS", "BUROCRACIA",
            "FILA DO INSS", "IMPOSTO", "PAUTA-BOMBA", "CPI",
    };

    /** Placas das barreiras baixas (pule). */
    static final String[] LOW_SIGNS = {"OBRA DO PAC", "EM OBRAS", "DESVIO", "PAC 3", "BURACO"};

    /** Letreiros dos onibus. */
    static final String[] BUS_DEST = {
            "GARANHUNS", "SÃO BERNARDO", "ESPLANADA", "PLANALTO", "ALVORADA", "ITAQUERA",
            "CAETÉS", "CONGRESSO", "VILA EUCLIDES",
    };

    static final String[] START = {
            "Companheiros e\ncompanheiras!", "Bora trabalhar,\ncompanheirada!", "O Brasil voltou!",
            "Cadê minha\npicanha?", "Hoje tem\nchurrasco!",
    };

    static final String[] COINS = {
            "Picanha pra\ntodo mundo!", "Isso é que é\nchurrasco!", "Picanha na mesa\ndo povo!",
            "Tá barato,\ncompanheiro!",
    };

    static final String[] STUMBLE = {"Olha o pato!", "Não vou pagar\no pato!", "Calma,\ncompanheiro!", "Opa! Fake news!"};

    static final String[] BUS = {"Ônibus lotado,\ncompanheiro!", "Esse vai pra\nGaranhuns!", "Vista linda\ndaqui de cima!"};

    static final String[] POW_NAME = {"ÍMÃ DO PIX", "AEROLULA", "CHINELO TURBO", "FAZ O L! 2X"};
    static final String[] POW_SAY = {
            "Faz o PIX\nda picanha!", "É pobre andando\nde avião!", "Chinelo turbo,\ncompanheiro!", "Faz o L!",
    };

    static final String[] CRASH_TITLE = {
            "PERDEU,\nCOMPANHEIRO!", "BATEU,\nMANÉ!", "ISSO FOI\nFAKE NEWS!", "AÍ NÃO,\nCOMPANHEIRO!",
            "DEU RUIM\nNA ESPLANADA!",
    };
    static final String[] CAUGHT_TITLE = {"O PATO\nTE PEGOU!", "PAGOU\nO PATO!", "QUEM PAGOU\nO PATO FOI VOCÊ!"};

    static final String RECORD = "NUNCA ANTES NA\nHISTÓRIA DESTE PAÍS!";

    static final String[] MENU_SUB = {
            "Picanha pra todos!", "O pato tá vindo!", "Faz o L e corre!", "Edição Companheirada",
            "Agora vai, companheiro!",
    };

    static final String[] SKIN_NAME = {"Terno Presidencial", "Metalúrgico do ABC", "Camisa Vermelha", "Torcedor do Timão", "Faixa Presidencial"};
    static final int[] SKIN_PRICE = {0, 300, 600, 1000, 2000};

    static String meta(int m) {
        switch (m) {
            case 1000: return "META ATINGIDA!";
            case 2000: return "DOBRAMOS A META!";
            case 4000: return "DOBRAMOS DE NOVO!";
            case 8000: return "META INFINITA!";
            default: return null;
        }
    }

    static String metaSay(int m) {
        switch (m) {
            case 1000: return "Agora vamos\ndobrar a meta!";
            case 2000: return "Quando atingir,\ndobra de novo!";
            case 4000: return "Não tem meta,\ntem companheirada!";
            default: return null;
        }
    }
}
