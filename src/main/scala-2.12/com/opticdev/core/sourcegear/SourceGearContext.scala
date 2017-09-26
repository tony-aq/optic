package com.opticdev.core.sourcegear

import com.opticdev.core.sourcegear.accumulate.FileAccumulator
import com.opticdev.parsers.{AstGraph, ParserBase}

case class SourceGearContext(var fileAccumulator: FileAccumulator,
                             var astGraph: AstGraph,
                             parser: ParserBase)
