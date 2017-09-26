package com.opticdev.core.sourcegear

import better.files.File
import com.opticdev.core.sourceparsers.SourceParserManager
import com.opticdev.parsers.ParserBase

import scalax.collection.edge.LkDiEdge
import scalax.collection.mutable.Graph

abstract class SourceGear {
  val parsers: Set[ParserBase]
  val gearSet: GearSet = new GearSet

  def parseFile(file: File) : Option[FileParseResults] = {
    val fileContents = file.contentAsString
    //@todo connect to parser list
    val parsedOption = SourceParserManager.parseString(fileContents, "Javascript", Option("es6"))

    if (parsedOption.isSuccess) {
      val parsed = parsedOption.get
      val astGraph = parsed.graph

      //@todo clean this up and have the parser return in the parse result.
      val parser = parsers.find(_.languageName == parsed.language).get

      implicit val sourceGearContext = SourceGearContext(gearSet.fileAccumulator, astGraph, parser)
      Option(gearSet.parseFromGraph(fileContents, astGraph, sourceGearContext))
    } else {
      None
    }

  }

}
